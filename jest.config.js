module.exports = {
  testEnvironment: 'node',
  transform: {
    '^.+\\.js$': 'babel-jest',
  },
  testMatch: ['**/tests/**/*.test.js'],
  testTimeout: 300000,
  verbose: true,
  moduleFileExtensions: ['js'],
  transformIgnorePatterns: [
    'node_modules/(?!(@solana)/)',
  ],
};
