# LeaseFlow Protocol - Error Code Reference

This document provides a comprehensive reference for all error codes that can be returned by the LeaseFlow Protocol smart contracts. Each error includes a code, technical message, and user-friendly explanation.

## Error Code Categories

### Authorization Errors (100-199)
These errors occur when a user attempts to perform an action without proper authorization.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 100 | UnauthorizedTenant | Only tenant can perform this action | You are not authorized as the tenant for this lease |
| 101 | UnauthorizedLandlord | Only landlord can perform this action | You are not authorized as the landlord for this property |
| 102 | UnauthorizedRegistryRemoval | Only landlord can remove from registry | Only the property owner can remove this listing |
| 103 | UpgradeNotAllowed | Upgrade logic mismatch or unauthorized | Contract upgrade failed due to terms mismatch or lack of permissions |

### Lease State Errors (200-299)
These errors occur when the lease is in an invalid state for the requested operation.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 200 | LeaseNotPending | Lease is not in pending state | This lease cannot be activated in its current state |
| 201 | LeaseNotActive | Lease is not active | This action requires an active lease |
| 202 | LeaseNotFound | Lease not found | No lease found for this property |
| 203 | LeaseAlreadyExists | Lease already exists | A lease already exists for this property |

### Property Registry Errors (300-399)
These errors are related to the global property registry system.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 300 | PropertyAlreadyLeased | Property already leased in another contract | This property is already leased to another tenant |
| 301 | PropertyNotRegistered | Property not found in global registry | This property is not registered in the system |
| 302 | InvalidPropertyHash | Invalid property hash format | Property identification data is corrupted |

### Financial Errors (400-499)
These errors occur during financial operations involving deposits and refunds.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 400 | DepositInsufficient | Security deposit is insufficient | Please add more funds to meet the security deposit requirement |
| 401 | InvalidRefundAmount | Invalid refund amount specified | The refund amount specified is not valid |
| 402 | RefundSumMismatch | Refund amounts must sum to total deposit | Refund amounts must exactly match the total security deposit |
| 403 | NegativeAmount | Amount cannot be negative | Amount values cannot be negative |

### Validation Errors (500-599)
These errors occur when input data fails validation.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 500 | InvalidDateRange | End date must be after start date | Please ensure the lease end date is after the start date |
| 501 | InvalidAddress | Invalid address format | The wallet address provided is not valid |
| 502 | InvalidSignature | Invalid signature provided | The signature verification failed |
| 503 | InvalidAmendment | Invalid lease amendment data | The lease amendment data is invalid or incomplete |

### System Errors (900-999)
These are internal system errors that typically require technical support.

| Code | Error Name | Technical Message | User-Friendly Message |
|------|------------|-------------------|----------------------|
| 900 | InternalError | Internal contract error occurred | A system error occurred. Please try again later |
| 901 | StorageError | Storage access error occurred | Data storage error. Please contact support |
| 902 | SerializationError | Data serialization error occurred | Data processing error. Please try again |

## Frontend Integration Guide

### Handling Errors in Frontend Applications

When interacting with the LeaseFlow contracts, frontend applications should:

1. **Parse Error Codes**: Extract the numeric error code from contract error messages
2. **Display User-Friendly Messages**: Use the user-friendly messages for better UX
3. **Implement Specific Actions**: Handle different error categories appropriately

### Example Error Handling Logic

```javascript
function handleContractError(error) {
    // Extract error code from message like "Error 300: Property already leased"
    const errorCodeMatch = error.message.match(/Error (\d+):/);
    
    if (errorCodeMatch) {
        const errorCode = parseInt(errorCodeMatch[1]);
        const errorInfo = getErrorInfo(errorCode);
        
        // Display user-friendly message
        showUserMessage(errorInfo.userFriendly);
        
        // Implement specific actions based on error category
        if (errorCode >= 100 && errorCode < 200) {
            // Authorization error - redirect to login
            redirectToLogin();
        } else if (errorCode >= 400 && errorCode < 500) {
            // Financial error - show payment interface
            showPaymentInterface();
        }
    } else {
        showUserMessage("An unexpected error occurred. Please try again.");
    }
}

function getErrorInfo(code) {
    const errorMap = {
        100: { userFriendly: "You are not authorized as the tenant for this lease" },
        300: { userFriendly: "This property is already leased to another tenant" },
        400: { userFriendly: "Please add more funds to meet the security deposit requirement" },
        // ... map all error codes
    };
    
    return errorMap[code] || { userFriendly: "An unknown error occurred" };
}
```

## Error Prevention Best Practices

### For Developers
1. **Validate Input Early**: Perform client-side validation before contract calls
2. **Check Lease Status**: Always verify lease state before performing actions
3. **Verify Authorization**: Ensure users have appropriate permissions
4. **Handle Edge Cases**: Account for all possible error scenarios

### For Users
1. **Complete Profile**: Ensure all required information is provided
2. **Sufficient Funds**: Maintain adequate balance for deposits and fees
3. **Valid Addresses**: Double-check wallet addresses before transactions
4. **Network Status**: Verify network connectivity before submitting transactions

## Testing Error Scenarios

The contract includes comprehensive invariant tests that verify:
- Error codes are returned correctly
- Error messages are descriptive
- Error handling maintains contract integrity
- User-friendly messages are appropriate

Run the test suite to verify error handling:
```bash
cargo test --workspace
```

## Support

For technical support regarding error codes or contract behavior:
1. Check this documentation first
2. Review the contract source code
3. Run the test suite for examples
4. Contact the development team with specific error codes and scenarios
