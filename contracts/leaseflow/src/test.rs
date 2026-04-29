#[cfg(test)]
mod test {
    use soroban_sdk::{Address, Bytes, Env, Vec};
    use crate::{LeaseFlowContract, LeaseState, Error, Lease, EscrowVault, ProtocolCreditRecord, MultiSigConfig, ProtocolFeeConfig, FeeUpdateProposal};

    #[test]
    fn test_lease_creation_and_states() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract
        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        let start_date = 1000;
        let end_date = 5000;
        let max_grace_period = 432000; // 5 days in seconds
        let late_fee_rate = 500; // 5% in basis points
        let property_uri = Bytes::from_slice(&env, b"property_uri");

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &max_grace_period,
            &late_fee_rate,
            &property_uri,
        );

        assert_eq!(lease_id, 1);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.lease_id, 1);
        assert_eq!(lease.state, LeaseState::Pending);
        assert_eq!(lease.max_grace_period, max_grace_period);
        assert_eq!(lease.late_fee_rate, late_fee_rate);
        assert!(!lease.arrears_processed);
    }

    #[test]
    fn test_grace_period_flow() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000, // 5 days
            &500,    // 5% late fee
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Verify active state
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::Active);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        let grace_lease = client.get_lease(&lease_id);
        assert_eq!(grace_lease.state, LeaseState::GracePeriod);
        assert!(grace_lease.dunning_start_timestamp.is_some());
        assert_eq!(grace_lease.outstanding_balance, 1000);
        assert_eq!(grace_lease.accumulated_late_fees, 50); // 5% of 1000

        // Recover during grace period
        client.process_rent_payment(&lease_id, &1050); // 1000 rent + 50 late fee

        let recovered_lease = client.get_lease(&lease_id);
        assert_eq!(recovered_lease.state, LeaseState::Active);
        assert_eq!(recovered_lease.outstanding_balance, 0);
        assert_eq!(recovered_lease.accumulated_late_fees, 0);
        assert!(recovered_lease.dunning_start_timestamp.is_none());
    }

    #[test]
    fn test_automated_arrears_deduction_basic() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        let late_fee_rate = 500; // 5%

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000, // 5 days
            &late_fee_rate,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Check initial escrow vault state
        let vault = client.get_escrow_vault();
        assert_eq!(vault.total_locked, deposit_amount);
        assert_eq!(vault.available_balance, deposit_amount);
        assert_eq!(vault.lessor_treasury, 0);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Simulate grace period expiry (this will auto-trigger arrears deduction)
        client.check_grace_period_expiry(&lease_id);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::EvictionPending);
        assert!(lease.arrears_processed);

        // Check escrow vault after deduction
        let vault_after = client.get_escrow_vault();
        let expected_deduction = rent_amount + (rent_amount * late_fee_rate as i64 / 10000);
        assert_eq!(vault_after.available_balance, deposit_amount - expected_deduction);
        assert_eq!(vault_after.lessor_treasury, expected_deduction);

        // Check credit record (should be none since deposit covered full arrears)
        let credit_record = client.get_credit_record(&lessee);
        assert!(credit_record.is_err()); // No residual debt
    }

    #[test]
    fn test_arrears_deduction_with_residual_debt() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 500; // Smaller deposit than total arrears
        let late_fee_rate = 500; // 5%

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &late_fee_rate,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Simulate grace period expiry (auto-triggers arrears deduction)
        client.check_grace_period_expiry(&lease_id);

        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::EvictionPending);
        assert!(lease.arrears_processed);

        // Check escrow vault - should be fully drained
        let vault_after = client.get_escrow_vault();
        assert_eq!(vault_after.available_balance, 0);
        assert_eq!(vault_after.lessor_treasury, deposit_amount);

        // Check credit record for residual debt
        let credit_record = client.get_credit_record(&lessee).unwrap();
        let total_arrears = rent_amount + (rent_amount * late_fee_rate as i64 / 10000);
        let expected_residual = total_arrears - deposit_amount;
        assert_eq!(credit_record.total_debt_amount, expected_residual);
        assert_eq!(credit_record.default_count, 1);
        assert!(credit_record.associated_lease_ids.contains(&lease_id));
    }

    #[test]
    fn test_manual_arrears_deduction() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;

        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Manually trigger grace period expiry
        client.check_grace_period_expiry(&lease_id);

        // Try to execute arrears deduction again (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err()); // Already processed

        // Verify state
        let lease = client.get_lease(&lease_id);
        assert!(lease.arrears_processed);
    }

    #[test]
    fn test_arrears_deduction_state_validation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        // Try to execute arrears deduction from Pending state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());

        // Activate lease
        client.activate_lease(&lease_id, &lessee);

        // Try from Active state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());

        // Trigger grace period
        client.handle_rent_payment_failure(&lease_id);

        // Try from GracePeriod state (should fail)
        let result = client.execute_arrears_deduction(&lease_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_credit_record_accumulation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 300; // Small to ensure residual debt

        // Create first lease
        let lease_id1 = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri1"),
        );

        client.activate_lease(&lease_id1, &lessee);
        client.handle_rent_payment_failure(&lease_id1);
        client.check_grace_period_expiry(&lease_id1);

        // Create second lease for same lessee
        let lease_id2 = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &6000,
            &10000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri2"),
        );

        client.activate_lease(&lease_id2, &lessee);
        client.handle_rent_payment_failure(&lease_id2);
        client.check_grace_period_expiry(&lease_id2);

        // Check accumulated credit record
        let credit_record = client.get_credit_record(&lessee).unwrap();
        assert_eq!(credit_record.default_count, 2);
        assert!(credit_record.associated_lease_ids.contains(&lease_id1));
        assert!(credit_record.associated_lease_ids.contains(&lease_id2));
        assert!(credit_record.total_debt_amount > 0);
    }

    #[test]
    fn test_prorated_rent_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 3100; // Amount divisible by 31 days
        let deposit_amount = 2000;
        
        // Create lease that starts in the past (mid-cycle scenario)
        let past_start = env.ledger().timestamp() - 5 * 86400; // Started 5 days ago
        let end_date = past_start + 31 * 86400; // 31-day lease
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &past_start,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        let lease = client.get_lease(&lease_id);
        
        // Should have prorated initial rent (26 days remaining out of 31)
        // 3100 * (26/31) = 2600
        assert_eq!(lease.prorated_initial_rent, 2600);
        assert_eq!(lease.total_paid_rent, 0);
    }

    #[test]
    fn test_prorated_rent_future_start() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        // Create lease that starts in the future
        let future_start = env.ledger().timestamp() + 10 * 86400; // Starts in 10 days
        let end_date = future_start + 30 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &future_start,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        let lease = client.get_lease(&lease_id);
        
        // Should have full rent (no proration for future start)
        assert_eq!(lease.prorated_initial_rent, rent_amount);
    }

    #[test]
    fn test_lease_termination_with_refund() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 3100; // Divisible by 31 days
        let deposit_amount = 2000;
        
        let start_date = env.ledger().timestamp();
        let end_date = start_date + 31 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Pay rent for tracking
        client.process_rent_payment(&lease_id, &rent_amount);
        
        // Advance time by 10 days
        env.ledger().set_timestamp(start_date + 10 * 86400);
        
        // Terminate lease (should refund for 21 remaining days)
        let refund = client.terminate_lease(&lease_id, &lessor);
        
        // Expected refund: 3100 * (21/31) = 2100, minus 1 stroop = 2099
        assert_eq!(refund, 2099);
        
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.state, LeaseState::Closed);
    }

    #[test]
    fn test_lease_termination_security_penalty() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        let start_date = env.ledger().timestamp();
        let end_date = start_date + 30 * 86400;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &start_date,
            &end_date,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        client.process_rent_payment(&lease_id, &rent_amount);
        
        // Terminate immediately (within 24 hours) - should apply penalty
        let refund = client.terminate_lease(&lease_id, &lessor);
        
        // Should apply 10% penalty for rapid termination
        // Full refund would be ~1000, penalty would be ~100, so refund ~900
        assert!(refund < 1000);
        assert!(refund > 800); // Should be reasonable
    }

    #[test]
    fn test_lease_termination_unauthorized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let unauthorized = Address::generate(&env);
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Try to terminate with unauthorized address
        let result = client.try_terminate_lease(&lease_id, &unauthorized);
        assert!(result.is_err());
    }

    #[test]
    fn test_prorated_rent_tracking() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        client.initialize();

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let rent_amount = 1000;
        let deposit_amount = 2000;
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &rent_amount,
            &deposit_amount,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &None, // No fiat peg
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Make multiple payments
        client.process_rent_payment(&lease_id, &rent_amount);
        client.process_rent_payment(&lease_id, &rent_amount);
        
        let lease = client.get_lease(&lease_id);
        assert_eq!(lease.total_paid_rent, 2000); // Should track total payments
    }

    #[test]
    fn test_fiat_pegged_lease_creation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100, // $100 USD target
            asset_address: asset_address.clone(),
            oracle_address: oracle_address.clone(),
            staleness_threshold: 900, // 15 minutes
            volatility_threshold: 2000, // 20%
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000, // Base rent in XLM
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        let lease = client.get_lease(&lease_id);
        assert!(lease.fiat_peg_config.is_some());
        let config = lease.fiat_peg_config.unwrap();
        assert_eq!(config.target_usd_amount, 100);
        assert_eq!(config.asset_address, asset_address);
        assert_eq!(config.oracle_address, oracle_address);
    }

    #[test]
    fn test_fiat_pegged_rent_calculation_bull_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle for bull market (XLM price increases from $0.10 to $0.20)
        let mock_oracle = MockSep40Oracle::new(&env);
        mock_oracle.set_price(&asset_address, &200000000, &7); // $0.20 with 7 decimals
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Process fiat-pegged rent - should require less XLM due to higher price
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        let lease = client.get_lease(&lease_id);
        // At $0.20 per XLM, $100 USD = 500 XLM (100 / 0.20)
        assert_eq!(lease.total_paid_rent, 500);
    }

    #[test]
    fn test_fiat_pegged_rent_calculation_bear_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle for bear market (XLM price drops to $0.05)
        let mock_oracle = MockSep40Oracle::new(&env);
        mock_oracle.set_price(&asset_address, &50000000, &7); // $0.05 with 7 decimals
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Process fiat-pegged rent - should require more XLM due to lower price
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        let lease = client.get_lease(&lease_id);
        // At $0.05 per XLM, $100 USD = 2000 XLM (100 / 0.05)
        assert_eq!(lease.total_paid_rent, 2000);
    }

    #[test]
    fn test_oracle_staleness_protection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        // Mock oracle with stale price (20 minutes old)
        let mock_oracle = MockSep40Oracle::new(&env);
        let stale_timestamp = env.ledger().timestamp() - 1200; // 20 minutes ago
        mock_oracle.set_price_with_timestamp(&asset_address, &100000000, &7, &stale_timestamp);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900, // 15 minutes
            volatility_threshold: 2000,
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Should fail due to stale oracle data
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::OracleDataStale);
    }

    #[test]
    fn test_volatility_circuit_breaker() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000, // 20%
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // First payment with normal price
        mock_oracle.set_price(&asset_address, &100000000, &7); // $0.10
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        // Advance time by 30 minutes
        env.ledger().set_timestamp(env.ledger().timestamp() + 1800);
        
        // Second payment with extreme price change (50% increase)
        mock_oracle.set_price(&asset_address, &150000000, &7); // $0.15 (+50%)
        
        // Should fail due to volatility circuit breaker
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::VolatilityCircuitBreaker);
    }

    #[test]
    fn test_12_month_lease_simulation_bull_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let start_time = env.ledger().timestamp();
        let end_time = start_time + 12 * 30 * 86400; // 12 months
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &start_time,
            &end_time,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Simulate 12 months of bull market (price increasing from $0.10 to $0.50)
        let mut total_paid = 0;
        for month in 0..12 {
            let price = 100000000 + (month as i128 * 33333333); // Linear increase
            mock_oracle.set_price(&asset_address, &price, &7);
            
            env.ledger().set_timestamp(start_time + (month + 1) * 30 * 86400);
            client.process_fiat_pegged_rent_payment(&lease_id);
            
            let lease = client.get_lease(&lease_id);
            total_paid = lease.total_paid_rent;
        }
        
        // In bull market, total XLM paid should decrease over time
        // Early months: ~1000 XLM, Later months: ~200 XLM
        assert!(total_paid < 12000); // Should be significantly less than fixed 12000 XLM
        assert!(total_paid > 2400);  // But still reasonable amount
    }

    #[test]
    fn test_12_month_lease_simulation_bear_market() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000,
        };
        
        let start_time = env.ledger().timestamp();
        let end_time = start_time + 12 * 30 * 86400; // 12 months
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &start_time,
            &end_time,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // Simulate 12 months of bear market (price decreasing from $0.10 to $0.02)
        let mut total_paid = 0;
        for month in 0..12 {
            let price = 100000000 - (month as i128 * 6666666); // Linear decrease
            mock_oracle.set_price(&asset_address, &price.max(20000000), &7);
            
            env.ledger().set_timestamp(start_time + (month + 1) * 30 * 86400);
            client.process_fiat_pegged_rent_payment(&lease_id);
            
            let lease = client.get_lease(&lease_id);
            total_paid = lease.total_paid_rent;
        }
        
        // In bear market, total XLM paid should increase over time
        // Early months: ~1000 XLM, Later months: ~5000 XLM
        assert!(total_paid > 12000); // Should be significantly more than fixed 12000 XLM
        assert!(total_paid < 60000); // But still reasonable
    }

    #[test]
    fn test_flash_loan_attack_protection() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        let oracle_address = Address::generate(&env);
        client.initialize_with_oracle(&oracle_address);

        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let asset_address = Address::generate(&env);
        
        let mock_oracle = MockSep40Oracle::new(&env);
        
        let fiat_peg_config = FiatPegConfig {
            enabled: true,
            target_usd_amount: 100,
            asset_address: asset_address.clone(),
            oracle_address: mock_oracle.address.clone(),
            staleness_threshold: 900,
            volatility_threshold: 2000, // 20% threshold
        };
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000,
            &2000,
            &1000,
            &5000,
            &432000,
            &500,
            &Bytes::from_slice(&env, b"property_uri"),
            &Some(fiat_peg_config),
        );

        client.activate_lease(&lease_id, &lessee);
        
        // First payment with normal price
        mock_oracle.set_price(&asset_address, &100000000, &7); // $0.10
        client.process_fiat_pegged_rent_payment(&lease_id);
        
        // Simulate flash loan attack - extreme price manipulation in same block
        mock_oracle.set_price(&asset_address, &50000000, &7); // 50% drop
        
        // Should fail due to volatility circuit breaker protecting against flash loan attacks
        let result = client.try_process_fiat_pegged_rent_payment(&lease_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), Error::VolatilityCircuitBreaker);
    }

    // Mock SEP-40 Oracle for testing
    struct MockSep40Oracle {
        env: Env,
        address: Address,
        prices: Map<Address, (i128, u32, u64)>, // (price, decimals, timestamp)
    }
    
    impl MockSep40Oracle {
        fn new(env: &Env) -> Self {
            let address = Address::generate(env);
            Self {
                env: env.clone(),
                address,
                prices: Map::new(env),
            }
        }
        
        fn set_price(&self, asset: &Address, price: &i128, decimals: &u32) {
            self.prices.set(asset, (*price, *decimals, self.env.ledger().timestamp()));
        }
        
        fn set_price_with_timestamp(&self, asset: &Address, price: &i128, decimals: &u32, timestamp: &u64) {
            self.prices.set(asset, (*price, *decimals, *timestamp));
        }
    }

    struct LeaseFlowContractClient<'a> {
        env: &'a Env,
        contract_id: &'a soroban_sdk::Address,
    }

    impl<'a> LeaseFlowContractClient<'a> {
        fn new(env: &'a Env, contract_id: &'a soroban_sdk::Address) -> Self {
            Self { env, contract_id }
        }

        fn initialize(&self) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("initialize"),
                soroban_sdk::xdr::ScVal::Void,
            );
        }

        fn initialize_with_oracle(&self, oracle_address: &Address) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("initialize"),
                soroban_sdk::xdr::ScVal::try_from(oracle_address).unwrap(),
            );
        }

        fn create_lease(
            &self,
            lessor: &Address,
            lessee: &Address,
            rent_amount: &i64,
            deposit_amount: &i64,
            start_date: &u64,
            end_date: &u64,
            max_grace_period: &u64,
            late_fee_rate: &u32,
            property_uri: &Bytes,
            fiat_peg_config: &Option<FiatPegConfig>,
        ) -> u64 {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("create_lease"),
                soroban_sdk::xdr::ScVal::try_from((
                    lessor, lessee, rent_amount, deposit_amount, 
                    start_date, end_date, max_grace_period, late_fee_rate, property_uri, fiat_peg_config
                )).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn activate_lease(&self, lease_id: &u64, lessee: &Address) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("activate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, lessee)).unwrap(),
            );
        }

        fn process_rent_payment(&self, lease_id: &u64, amount: &i64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, amount)).unwrap(),
            );
        }

        fn handle_rent_payment_failure(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("handle_rent_payment_failure"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn check_grace_period_expiry(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("check_grace_period_expiry"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn execute_arrears_deduction(&self, lease_id: &u64) -> Result<(), Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("execute_arrears_deduction"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into()
        }

        fn get_lease(&self, lease_id: &u64) -> Lease {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_lease"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn get_credit_record(&self, lessee: &Address) -> Result<ProtocolCreditRecord, Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_credit_record"),
                soroban_sdk::xdr::ScVal::try_from(lessee).unwrap(),
            );
            result.try_into()
        }

        fn get_escrow_vault(&self) -> EscrowVault {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("get_escrow_vault"),
                soroban_sdk::xdr::ScVal::Void,
            );
            result.try_into().unwrap()
        }

        fn terminate_lease(&self, lease_id: &u64, caller: &Address) -> i64 {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("terminate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, caller)).unwrap(),
            );
            result.try_into().unwrap()
        }

        fn try_terminate_lease(&self, lease_id: &u64, caller: &Address) -> Result<i64, Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("terminate_lease"),
                soroban_sdk::xdr::ScVal::try_from((lease_id, caller)).unwrap(),
            );
            result.try_into()
        }

        fn process_fiat_pegged_rent_payment(&self, lease_id: &u64) {
            self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_fiat_pegged_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
        }

        fn try_process_fiat_pegged_rent_payment(&self, lease_id: &u64) -> Result<(), Error> {
            let result = self.env.invoke_contract(
                self.contract_id,
                &soroban_sdk::symbol!("process_fiat_pegged_rent_payment"),
                soroban_sdk::xdr::ScVal::try_from(lease_id).unwrap(),
            );
            result.try_into()
        }
    }

    // Multi-signature fee validation tests
    #[test]
    fn test_initialize_multisig() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract
        client.initialize();

        // Create signatories
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        let signatory3 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());
        signatories.push_back(signatory3.clone());

        // Initialize multi-sig configuration
        client.initialize_multisig(
            &signatories,
            &2, // threshold of 2 signatures
            &86400, // 24 hour timelock
            &3000, // max 30% fee
            &100, // min 1% fee
            &500, // max 5% increase per update
            &200, // initial 2% fee
        );

        // Verify configuration
        let config = client.get_multisig_config();
        assert_eq!(config.threshold, 2);
        assert_eq!(config.timelock_period, 86400);
        assert_eq!(config.signatories.len(), 3);

        // Verify protocol fee configuration
        let fee_config = client.get_protocol_fee_config();
        assert_eq!(fee_config.protocol_fee_bps, 200);
        assert_eq!(fee_config.updated_by, signatory1);
    }

    #[test]
    fn test_propose_fee_update() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &86400,
            &3000,
            &100,
            &500,
            &200,
        );

        // Propose fee update
        let description = Bytes::from_slice(&env, b"Increase fee to 3%");
        let proposal_id = client.propose_fee_update(
            &signatory1,
            &300, // 3% fee
            &description,
        );

        // Verify proposal
        let proposal = client.get_fee_proposal(&proposal_id);
        assert_eq!(proposal.proposed_fee_bps, 300);
        assert_eq!(proposal.proposed_by, signatory1);
        assert!(!proposal.executed);
        assert_eq!(proposal.signatures.len(), 0);
    }

    #[test]
    fn test_sign_fee_proposal() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &86400,
            &3000,
            &100,
            &500,
            &200,
        );

        // Create and sign proposal
        let description = Bytes::from_slice(&env, b"Increase fee to 3%");
        let proposal_id = client.propose_fee_update(&signatory1, &300, &description);
        
        // First signature
        client.sign_fee_proposal(&signatory1, &proposal_id);
        let proposal = client.get_fee_proposal(&proposal_id);
        assert_eq!(proposal.signatures.len(), 1);
        assert!(proposal.signatures.contains(&signatory1));

        // Second signature
        client.sign_fee_proposal(&signatory2, &proposal_id);
        let proposal = client.get_fee_proposal(&proposal_id);
        assert_eq!(proposal.signatures.len(), 2);
        assert!(proposal.signatures.contains(&signatory2));

        // Test double-signing prevention
        let result = client.try_sign_fee_proposal(&signatory1, &proposal_id);
        assert_eq!(result, Err(Error::AlreadySigned));
    }

    #[test]
    fn test_execute_fee_update() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &1, // 1 second timelock for testing
            &3000,
            &100,
            &500,
            &200,
        );

        // Create, sign, and execute proposal
        let description = Bytes::from_slice(&env, b"Increase fee to 3%");
        let proposal_id = client.propose_fee_update(&signatory1, &300, &description);
        
        client.sign_fee_proposal(&signatory1, &proposal_id);
        client.sign_fee_proposal(&signatory2, &proposal_id);

        // Wait for timelock
        env.ledger().set_timestamp(env.ledger().timestamp() + 2);

        // Execute proposal
        client.execute_fee_update(&signatory1, &proposal_id);

        // Verify fee was updated
        let fee_config = client.get_protocol_fee_config();
        assert_eq!(fee_config.protocol_fee_bps, 300);
        assert_eq!(fee_config.updated_by, signatory1);

        // Verify proposal is marked as executed
        let proposal = client.get_fee_proposal(&proposal_id);
        assert!(proposal.executed);
    }

    #[test]
    fn test_fee_validation_limits() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        signatories.push_back(signatory1.clone());

        client.initialize_multisig(
            &signatories,
            &1,
            &86400,
            &3000, // max 30%
            &100,  // min 1%
            &500,  // max 5% increase
            &200,  // initial 2%
        );

        // Test exceeding max fee
        let description = Bytes::from_slice(&env, b"Excessive fee");
        let result = client.try_propose_fee_update(&signatory1, &4000, &description);
        assert_eq!(result, Err(Error::ExceedsMaxFee));

        // Test below min fee
        let result = client.try_propose_fee_update(&signatory1, &50, &description);
        assert_eq!(result, Err(Error::BelowMinFee));

        // Test exceeding increase limit
        let result = client.try_propose_fee_update(&signatory1, &800, &description); // 6% increase
        assert_eq!(result, Err(Error::InvalidFeeChange));
    }

    #[test]
    fn test_multisig_authorization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &86400,
            &3000,
            &100,
            &500,
            &200,
        );

        // Test unauthorized proposal
        let unauthorized = Address::generate(&env);
        let description = Bytes::from_slice(&env, b"Unauthorized proposal");
        let result = client.try_propose_fee_update(&unauthorized, &300, &description);
        assert_eq!(result, Err(Error::InvalidSignatory));

        // Test unauthorized signing
        let proposal_id = client.propose_fee_update(&signatory1, &300, &description);
        let result = client.try_sign_fee_proposal(&unauthorized, &proposal_id);
        assert_eq!(result, Err(Error::InvalidSignatory));
    }

    #[test]
    fn test_protocol_fee_integration() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        signatories.push_back(signatory1.clone());

        client.initialize_multisig(
            &signatories,
            &1,
            &1,
            &3000,
            &100,
            &500,
            &500, // 5% protocol fee
        );

        // Create lease
        let lessor = Address::generate(&env);
        let lessee = Address::generate(&env);
        let property_uri = Bytes::from_slice(&env, b"property_uri");
        
        let lease_id = client.create_lease(
            &lessor,
            &lessee,
            &1000, // rent amount
            &2000, // deposit
            &1000, // start date
            &5000, // end date
            &432000, // grace period
            &300, // late fee (within protocol limits)
            &property_uri,
        );

        // Activate lease
        client.activate_lease(&lease_id, &lessee);

        // Process rent payment (protocol fee should be deducted)
        client.process_rent_payment(&lease_id, &1000);

        // Check escrow vault for protocol fee collection
        let vault = client.get_escrow_vault();
        let expected_protocol_fee = 1000 * 500 / 10000; // 5% of 1000 = 50
        assert_eq!(vault.lessor_treasury, expected_protocol_fee);
    }

    #[test]
    fn test_emergency_fee_update() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &86400,
            &3000,
            &100,
            &500,
            &200,
        );

        // Test emergency update by first signatory
        let reason = Bytes::from_slice(&env, b"Emergency update");
        client.emergency_fee_update(&signatory1, &400, &reason);

        // Verify fee was updated
        let fee_config = client.get_protocol_fee_config();
        assert_eq!(fee_config.protocol_fee_bps, 400);

        // Test unauthorized emergency update
        let result = client.try_emergency_fee_update(&signatory2, &500, &reason);
        assert_eq!(result, Err(Error::Unauthorized));
    }

    #[test]
    fn test_update_signatory() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LeaseFlowContract);
        let client = LeaseFlowContractClient::new(&env, &contract_id);

        // Initialize contract and multi-sig
        client.initialize();
        
        let mut signatories = Vec::new(&env);
        let signatory1 = Address::generate(&env);
        let signatory2 = Address::generate(&env);
        signatories.push_back(signatory1.clone());
        signatories.push_back(signatory2.clone());

        client.initialize_multisig(
            &signatories,
            &2,
            &86400,
            &3000,
            &100,
            &500,
            &200,
        );

        // Update signatory
        let new_signatory = Address::generate(&env);
        client.update_signatory(&signatory1, &signatory2.clone(), &new_signatory.clone());

        // Verify signatory was updated
        let config = client.get_multisig_config();
        assert!(config.signatories.contains(&signatory1));
        assert!(config.signatories.contains(&new_signatory));
        assert!(!config.signatories.contains(&signatory2));
    }
}

//! LeaseFlow – Test Suite
//!
//! Tests are organised by state transition.  Every test verifies three things:
//!   1. The on-chain `Lease.status` changed to the expected state.
//!   2. The correct contract **event** was emitted (topics + payload).
//!   3. The aggregate **metrics** counter was incremented.
//!
//! Run with:
//!   ```
//!   cargo test --package lease-flow -- --nocapture
//!   ```

#![cfg(test)]

extern crate std;
use std::println;

use soroban_sdk::{
    testutils::{Events, Ledger, LedgerInfo},
    vec, Address, Env, IntoVal, Symbol,
};

use crate::{
    DataKey, LeaseFlowContract, LeaseFlowContractClient,
    LeaseState, StateTransitionEvent, TransitionMetrics,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a fresh test environment with a deterministic ledger sequence.
fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        timestamp:          1_700_000_000,
        protocol_version:   22,
        sequence_number:    100,
        network_id:         Default::default(),
        base_reserve:       10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl:      6_312_000,
    });
    env
}

/// Register the contract and return a typed client.
fn deploy(env: &Env) -> LeaseFlowContractClient {
    let contract_id = env.register(LeaseFlowContract, ());
    LeaseFlowContractClient::new(env, &contract_id)
}

/// Deterministic addresses for actors.
fn landlord(env: &Env) -> Address { Address::generate(env) }
fn tenant(env: &Env)   -> Address { Address::generate(env) }

/// Create a standard lease; returns (lease_id, landlord, tenant).
fn make_lease(
    client:   &LeaseFlowContractClient,
    env:      &Env,
) -> (u64, Address, Address) {
    let ll = landlord(env);
    let tt = tenant(env);

    let id = client.create_lease(
        &ll,
        &tt,
        &1_000_i128,   // rent_amount
        &5_000_i128,   // deposit_amount
        &1_700_000_000_u64,
        &1_702_678_400_u64,
        &soroban_sdk::String::from_str(env, "ipfs://QmTest"),
    );

    (id, ll, tt)
}

/// Pull the last emitted event payload and assert its fields.
fn last_transition_event(env: &Env) -> StateTransitionEvent {
    let all_events = env.events().all();
    // The last event in the list is the most recent.
    let (_cid, _topics, data) = all_events.last().unwrap();
    data.into_val(env)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. CREATE LEASE → Pending
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_create_lease_emits_created_event() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, _tt) = make_lease(&client, &env);

    println!("[test_create_lease] lease_id={id}");

    // 1a. Lease stored as Pending
    let lease = client.get_lease(&id);
    assert_eq!(lease.status, LeaseState::Pending, "status should be Pending after creation");

    // 1b. Event payload
    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.lease_id,   id);
    assert_eq!(evt.to_state,   LeaseState::Pending);
    assert_eq!(evt.actor,      ll);
    assert_eq!(evt.reason,     Symbol::new(&env, "created"));

    println!("[test_create_lease] ✓ event emitted: reason={:?}", evt.reason);

    // 1c. No metric counter for creation (by design – creation is not a transition)
    let m = client.get_metrics();
    assert_eq!(m.total_transitions, 0, "no metric counter on creation");
}

#[test]
fn test_lease_counter_increments() {
    let env    = setup_env();
    let client = deploy(&env);

    assert_eq!(client.get_lease_count(), 0);
    make_lease(&client, &env);
    assert_eq!(client.get_lease_count(), 1);
    make_lease(&client, &env);
    assert_eq!(client.get_lease_count(), 2);

    println!("[test_lease_counter] ✓ counter increments correctly");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. ACTIVATE LEASE  (Pending → Active)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_activate_lease_emits_activated_event_and_increments_metric() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);

    println!("[test_activate] lease_id={id}");

    // 2a. Status flipped to Active
    let lease = client.get_lease(&id);
    assert_eq!(lease.status, LeaseState::Active);
    assert_eq!(lease.deposit_paid, 5_000);

    // 2b. Event
    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Pending);
    assert_eq!(evt.to_state,   LeaseState::Active);
    assert_eq!(evt.actor,      tt);
    assert_eq!(evt.reason,     Symbol::new(&env, "activated"));

    // 2c. Metric
    let m = client.get_metrics();
    assert_eq!(m.pending_to_active,  1);
    assert_eq!(m.total_transitions,  1);

    println!("[test_activate] ✓ metric pending_to_active={}", m.pending_to_active);
}

#[test]
#[should_panic(expected = "lease must be Pending to activate")]
fn test_activate_already_active_lease_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    // Second activation must fail
    client.activate_lease(&id, &tt, &5_000_i128);
}

#[test]
#[should_panic(expected = "deposit too low")]
fn test_activate_with_insufficient_deposit_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &100_i128); // deposit_amount is 5_000
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. TRIGGER EVICTION  (Active → Eviction)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_trigger_eviction_emits_eviction_event_and_increments_metric() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.trigger_eviction(&id, &ll);

    println!("[test_eviction] lease_id={id}");

    // 3a. Status
    assert_eq!(client.get_lease(&id).status, LeaseState::Eviction);

    // 3b. Event
    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Active);
    assert_eq!(evt.to_state,   LeaseState::Eviction);
    assert_eq!(evt.reason,     Symbol::new(&env, "eviction"));

    // 3c. Metric
    let m = client.get_metrics();
    assert_eq!(m.active_to_eviction, 1);
    assert_eq!(m.total_transitions,  2); // activate + eviction

    println!("[test_eviction] ✓ metric active_to_eviction={}", m.active_to_eviction);
}

#[test]
#[should_panic(expected = "can only evict an Active lease")]
fn test_evict_pending_lease_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, _tt) = make_lease(&client, &env);

    client.trigger_eviction(&id, &ll);
}

#[test]
#[should_panic(expected = "only the landlord may trigger eviction")]
fn test_eviction_by_non_landlord_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);
    let impostor = Address::generate(&env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.trigger_eviction(&id, &impostor);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. MARK DEFAULTED  (Active → Defaulted)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mark_defaulted_emits_defaulted_event_and_increments_metric() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.mark_defaulted(&id, &ll);

    println!("[test_defaulted] lease_id={id}");

    assert_eq!(client.get_lease(&id).status, LeaseState::Defaulted);

    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Active);
    assert_eq!(evt.to_state,   LeaseState::Defaulted);
    assert_eq!(evt.reason,     Symbol::new(&env, "defaulted"));

    let m = client.get_metrics();
    assert_eq!(m.active_to_defaulted, 1);
    assert_eq!(m.total_transitions,   2);

    println!("[test_defaulted] ✓ metric active_to_defaulted={}", m.active_to_defaulted);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. RAISE DISPUTE  (Active → Disputed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_raise_dispute_emits_disputed_event_and_increments_metric() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.raise_dispute(&id, &ll);

    println!("[test_disputed] lease_id={id}");

    assert_eq!(client.get_lease(&id).status, LeaseState::Disputed);

    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Active);
    assert_eq!(evt.to_state,   LeaseState::Disputed);
    assert_eq!(evt.reason,     Symbol::new(&env, "disputed"));

    let m = client.get_metrics();
    assert_eq!(m.active_to_disputed, 1);
    assert_eq!(m.total_transitions,  2);

    println!("[test_disputed] ✓ metric active_to_disputed={}", m.active_to_disputed);
}

#[test]
#[should_panic(expected = "only the landlord may raise a dispute")]
fn test_raise_dispute_by_tenant_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.raise_dispute(&id, &tt);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. CLOSE LEASE – from every valid source state
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_close_from_active_increments_active_to_closed() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.close_lease(&id, &ll);

    println!("[test_close_active] lease_id={id}");

    assert_eq!(client.get_lease(&id).status, LeaseState::Closed);

    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Active);
    assert_eq!(evt.to_state,   LeaseState::Closed);
    assert_eq!(evt.reason,     Symbol::new(&env, "closed"));

    let m = client.get_metrics();
    assert_eq!(m.active_to_closed,  1);
    assert_eq!(m.total_transitions, 2);

    println!("[test_close_active] ✓ metric active_to_closed={}", m.active_to_closed);
}

#[test]
fn test_close_from_eviction_increments_eviction_to_closed() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.trigger_eviction(&id, &ll);
    client.close_lease(&id, &ll);

    let m = client.get_metrics();
    assert_eq!(m.eviction_to_closed, 1, "eviction_to_closed should be 1");

    println!("[test_close_eviction] ✓ metric eviction_to_closed={}", m.eviction_to_closed);
}

#[test]
fn test_close_from_disputed_increments_disputed_to_closed() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.raise_dispute(&id, &ll);
    client.close_lease(&id, &ll);

    let m = client.get_metrics();
    assert_eq!(m.disputed_to_closed, 1);

    println!("[test_close_disputed] ✓ metric disputed_to_closed={}", m.disputed_to_closed);
}

#[test]
fn test_close_from_defaulted_increments_defaulted_to_closed() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.mark_defaulted(&id, &ll);
    client.close_lease(&id, &ll);

    let m = client.get_metrics();
    assert_eq!(m.defaulted_to_closed, 1);

    println!("[test_close_defaulted] ✓ metric defaulted_to_closed={}", m.defaulted_to_closed);
}

#[test]
#[should_panic(expected = "lease cannot be closed from current state")]
fn test_close_already_closed_lease_panics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.close_lease(&id, &ll);
    client.close_lease(&id, &ll); // double-close must fail
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. STREAM RENT – auto-eviction when funds exhausted
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_stream_rent_auto_triggers_eviction_when_deposit_exhausted() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    // deposit_amount = 5_000; stream 5_000 in one call → auto-eviction
    client.activate_lease(&id, &tt, &5_000_i128);
    client.stream_rent(&id, &ll, &5_000_i128);

    println!("[test_stream_auto_evict] lease_id={id}");

    assert_eq!(client.get_lease(&id).status, LeaseState::Eviction);

    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.from_state, LeaseState::Active);
    assert_eq!(evt.to_state,   LeaseState::Eviction);
    assert_eq!(evt.reason,     Symbol::new(&env, "eviction"));

    let m = client.get_metrics();
    assert_eq!(m.active_to_eviction, 1);

    println!("[test_stream_auto_evict] ✓ auto-eviction triggered, metric active_to_eviction={}", m.active_to_eviction);
}

#[test]
fn test_stream_rent_partial_does_not_trigger_eviction() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.stream_rent(&id, &ll, &1_000_i128);

    let lease = client.get_lease(&id);
    assert_eq!(lease.status,          LeaseState::Active);
    assert_eq!(lease.streamed_amount,  1_000);

    let m = client.get_metrics();
    assert_eq!(m.active_to_eviction, 0, "no eviction on partial stream");

    println!("[test_stream_partial] ✓ partial stream, lease still Active");
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. FULL HAPPY PATH  (Pending → Active → Closed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_full_happy_path_metrics_accumulate_correctly() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    // Step 1: activate
    client.activate_lease(&id, &tt, &5_000_i128);
    // Step 2: stream some rent
    client.stream_rent(&id, &ll, &2_000_i128);
    // Step 3: close
    client.close_lease(&id, &ll);

    println!("[test_happy_path] lease_id={id}");

    let lease = client.get_lease(&id);
    assert_eq!(lease.status,          LeaseState::Closed);
    assert_eq!(lease.streamed_amount,  2_000);

    let m = client.get_metrics();
    // activate + close = 2 total
    assert_eq!(m.total_transitions,  2);
    assert_eq!(m.pending_to_active,  1);
    assert_eq!(m.active_to_closed,   1);
    assert_eq!(m.active_to_eviction, 0);

    println!(
        "[test_happy_path] ✓ totals: transitions={}, pending_to_active={}, active_to_closed={}",
        m.total_transitions, m.pending_to_active, m.active_to_closed
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. FULL DISPUTED PATH  (Pending → Active → Disputed → Closed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_disputed_path_metrics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.raise_dispute(&id, &ll);
    client.close_lease(&id, &ll);

    let m = client.get_metrics();
    assert_eq!(m.total_transitions,  3);
    assert_eq!(m.pending_to_active,  1);
    assert_eq!(m.active_to_disputed, 1);
    assert_eq!(m.disputed_to_closed, 1);

    println!(
        "[test_disputed_path] ✓ transitions={} (activate + dispute + close)",
        m.total_transitions
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. FULL EVICTION PATH  (Pending → Active → Eviction → Defaulted → Closed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_eviction_then_default_path_metrics() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, ll, tt) = make_lease(&client, &env);

    client.activate_lease(&id, &tt, &5_000_i128);
    client.trigger_eviction(&id, &ll);
    client.mark_defaulted(&id, &ll);
    client.close_lease(&id, &ll);

    let m = client.get_metrics();
    assert_eq!(m.total_transitions,   4);
    assert_eq!(m.pending_to_active,   1);
    assert_eq!(m.active_to_eviction,  1);
    assert_eq!(m.active_to_defaulted, 1); // from Eviction state
    assert_eq!(m.defaulted_to_closed, 1);

    println!(
        "[test_eviction_default_path] ✓ full eviction path, transitions={}",
        m.total_transitions
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 11. MULTI-LEASE ISOLATION
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_multiple_leases_share_global_metrics() {
    let env    = setup_env();
    let client = deploy(&env);

    // Lease A: happy path
    let (id_a, ll_a, tt_a) = make_lease(&client, &env);
    client.activate_lease(&id_a, &tt_a, &5_000_i128);
    client.close_lease(&id_a, &ll_a);

    // Lease B: disputed path
    let (id_b, ll_b, tt_b) = make_lease(&client, &env);
    client.activate_lease(&id_b, &tt_b, &5_000_i128);
    client.raise_dispute(&id_b, &ll_b);
    client.close_lease(&id_b, &ll_b);

    // Lease C: eviction path
    let (id_c, ll_c, tt_c) = make_lease(&client, &env);
    client.activate_lease(&id_c, &tt_c, &5_000_i128);
    client.trigger_eviction(&id_c, &ll_c);
    client.close_lease(&id_c, &ll_c);

    println!("[test_multi_lease] lease_ids: {id_a}, {id_b}, {id_c}");

    let m = client.get_metrics();
    // 3 activations + 3 closes + 1 dispute + 1 eviction = 8
    assert_eq!(m.total_transitions,  8);
    assert_eq!(m.pending_to_active,  3);
    assert_eq!(m.active_to_closed,   1);
    assert_eq!(m.active_to_disputed, 1);
    assert_eq!(m.active_to_eviction, 1);
    assert_eq!(m.disputed_to_closed, 1);
    assert_eq!(m.eviction_to_closed, 1);

    // Leases are independent
    assert_eq!(client.get_lease(&id_a).status, LeaseState::Closed);
    assert_eq!(client.get_lease(&id_b).status, LeaseState::Closed);
    assert_eq!(client.get_lease(&id_c).status, LeaseState::Closed);

    println!(
        "[test_multi_lease] ✓ global totals: transitions={}, pending_to_active={}, \
         active_to_eviction={}, active_to_disputed={}, eviction_to_closed={}, \
         disputed_to_closed={}",
        m.total_transitions, m.pending_to_active, m.active_to_eviction,
        m.active_to_disputed, m.eviction_to_closed, m.disputed_to_closed,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 12. EVENT PAYLOAD TIMESTAMP MATCHES LEDGER
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_event_timestamp_matches_ledger_sequence() {
    let env    = setup_env();
    let client = deploy(&env);
    let (id, _ll, tt) = make_lease(&client, &env);

    // Advance the ledger
    env.ledger().set(LedgerInfo {
        timestamp:          1_700_000_500,
        protocol_version:   22,
        sequence_number:    250,
        network_id:         Default::default(),
        base_reserve:       10,
        min_temp_entry_ttl: 16,
        min_persistent_entry_ttl: 4096,
        max_entry_ttl:      6_312_000,
    });

    client.activate_lease(&id, &tt, &5_000_i128);

    let evt: StateTransitionEvent = last_transition_event(&env);
    assert_eq!(evt.timestamp, 250, "timestamp should match ledger sequence_number=250");

    println!("[test_timestamp] ✓ event.timestamp={} matches ledger.sequence=250", evt.timestamp);
}