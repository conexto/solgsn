# Integration Tests

This directory contains integration tests for SolGSN that test the complete flow from localnet setup to program execution.

## Prerequisites

1. **Localnet**: Solana localnet must be running
2. **Program Built**: The Rust program must be compiled to `dist/program/solgsn.so`

## Running Tests

### Option 1: Manual Setup

```bash
# 1. Start localnet
npm run localnet:up

# 2. Build the program
npm run build

# 3. Run integration tests
npm run test:integration
```

### Option 2: Reset and Test

```bash
# This will reset localnet, clean store, start localnet, build, and then you can test
npm run localnet:reset
npm run test:integration
```

## Test Coverage

The integration tests cover the following flows:

1. **User top-up with SOL**: Tests that users can top up their GSN account with SOL
2. **User top-up with SPL token (mock)**: Tests SPL token top-up (currently mocked, as full SPL integration requires additional implementation)
3. **Gasless transaction with fee deduction**: Tests that a gasless transaction successfully executes and deducts fees from the user's top-up balance
4. **Executor claiming fees**: Tests that executors can claim their accumulated fees
5. **User withdrawal**: Documents the expected withdrawal flow (not yet implemented in the program)

## Test Structure

- `integration.test.js`: Main test file containing all integration tests
- Tests use Jest as the test framework
- Each test has a 5-minute timeout to account for blockchain operations
- Tests automatically set up localnet connection, deploy program, and initialize GSN account

## Notes

- **SPL Token Support**: The current implementation tracks amounts but doesn't perform actual SPL token transfers. Full SPL token support would require integration with the SPL Token Program.
- **Withdrawal**: The withdrawal functionality is planned but not yet implemented in the program. The test documents the expected behavior.
- **State Reading**: The tests use simplified state reading. In production, you would use proper Borsh deserialization to read the GSN state account.

## Troubleshooting

### Localnet not ready
If you see "Localnet not ready after max retries", ensure:
- Localnet is running: `npm run localnet:up`
- Wait a few seconds for it to fully start
- Check logs: `npm run localnet:logs`

### Program not found
If you see "Program not found" errors:
- Build the program: `npm run build`
- Ensure `dist/program/solgsn.so` exists

### Test timeouts
If tests timeout:
- Increase timeout in `jest.config.js`
- Check localnet is responsive
- Verify sufficient funds in payer account
