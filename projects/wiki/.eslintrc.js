module.exports = {
  plugins: ['eslint-plugin-unused-imports', '@typescript-eslint', 'functional'],
  extends: [
    'plugin:functional/recommended',
    'plugin:functional/stylistic',
    'plugin:react-hooks/recommended',
    'next/core-web-vitals',
    'prettier',
  ],
  ignorePatterns: ['schema.gql', 'schema.generated.ts'],
  rules: {
    'functional/prefer-immutable-types': 'off',
    'functional/prefer-readonly-type': 'off',
    'functional/functional-parameters': 'off',
    'functional/no-conditional-statements': 'off',
    'functional/no-return-void': 'off',
    'functional/no-expression-statements': 'off',
    'functional/no-mixed-types': 'off',
    '@typescript-eslint/no-explicit-any': 'error',
    'react-hooks/exhaustive-deps': 'off',
    'unused-imports/no-unused-imports': 'error',
    'no-restricted-syntax': [
      'error',
      'FunctionExpression',
      'FunctionDeclaration',
    ],
    '@typescript-eslint/consistent-type-definitions': ['error', 'type'],
  },
  parser: '@typescript-eslint/parser',
  parserOptions: {
    project: './tsconfig.json',
    tsconfigRootDir: __dirname,
  },
};
