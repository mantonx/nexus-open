const { builtinModules } = require('module')

module.exports = {
  root: true,
  extends: [
    'prettier',
    'react-app',
    'plugin:react-hooks/recommended',
    'plugin:chai-friendly/recommended',
    'plugin:cypress/recommended',
  ],
  plugins: [
    '@typescript-eslint',
    'prettier',
    'simple-import-sort',
    'lodash',
    'cypress',
    'chai-friendly',
    'import',
  ],
  parser: '@typescript-eslint/parser',
  settings: {
    react: {
      version: 'detect',
    },
    'import/resolver': {
      node: {
        extensions: ['.ts', '.tsx'],
        preserveSymlinks: false,
      },
    },
  },
  rules: {
    'cypress/unsafe-to-chain-command': 2,
    'array-callback-return': 'error',
    eqeqeq: 'error',
    'for-direction': 'error',
    'no-class-assign': 'error',
    'no-debugger': 'error',
    'no-dupe-class-members': 'error',
    'no-duplicate-case': 'error',
    'no-empty-pattern': 'error',
    'no-fallthrough': 'error',
    'no-func-assign': 'error',
    'no-loop-func': 'error',
    'no-redeclare': 'off',
    '@typescript-eslint/no-redeclare': ['error'],
    'no-restricted-globals': 'off',
    'no-restricted-imports': [
      'error',
      {
        patterns: [
          {
            message: "Use '@/proto/...' instead",
            group: [
              '**/../lib/axios/**',
              '**/../lib/google/**',
              '**/lib/typescript/src/**',
              '@proto/**',
            ],
          },
        ],
      },
    ],
    'no-unreachable': 'error',
    'no-undef': 'off',
    'no-unused-vars': 'off',
    'no-unused-expressions': 'off',
    'no-useless-escape': 'error',
    '@typescript-eslint/no-unused-vars': [
      'error',
      {
        argsIgnorePattern: '^_',
        caughtErrors: 'all',
        caughtErrorsIgnorePattern: '^_',
        ignoreRestSiblings: true,
        varsIgnorePattern: '^_',
      },
    ],
    '@typescript-eslint/no-unused-expressions': 'off',
    'chai-friendly/no-unused-expressions': 'error',
    'no-use-before-define': 'off',
    'prefer-const': [
      'error',
      {
        destructuring: 'all',
      },
    ],
    'quote-props': ['error', 'as-needed'],
    'use-isnan': 'error',
    'import/default': 'error',
    'import/export': 'error',
    'import/exports-last': 'error',
    'import/first': 'error',
    'import/named': 'error',
    'import/namespace': 'error',
    'import/newline-after-import': 'error',
    'import/no-anonymous-default-export': ['error', { allowArrowFunction: true }],
    'import/no-duplicates': 'error',
    'import/no-extraneous-dependencies': ['error'],
    'import/no-named-as-default': 'error',
    'import/no-named-as-default-member': 'error',
    'import/no-named-default': 'error',
    'import/no-namespace': 'error',
    'import/no-restricted-paths': [
      'error',
      {
        zones: [
          {
            target: '**/axios-render/**/*',
            from: '**/axios-web/**/*',
            message: "Don't import from the axios-web service into the axios-render service.",
          },
          {
            target: '**/lib/typescript/src/tiptap/**/*',
            from: '**/axios-web/**/*',
            message: "Don't import the axios-web service into the shared Tiptap library.",
          },
        ],
      },
    ],
    'jsx-a11y/anchor-is-valid': [
      'error',
      {
        aspects: ['invalidHref'],
      },
    ],
    'jsx-a11y/heading-has-content': 'off',
    'jsx-a11y/href-no-hash': 'off',
    'lodash/import-scope': 'error',
    'prettier/prettier': [
      'error',
      {
        printWidth: 100,
        semi: false,
        singleQuote: true,
        trailingComma: 'es5',
        bracketSpacing: true,
        arrowParens: 'always',
      },
    ],
    'react/default-props-match-prop-types': 'error',
    'react/no-unknown-property': 'error',
    'simple-import-sort/exports': 'error',
    'simple-import-sort/imports': [
      'error',
      {
        groups: [
          // side-effect imports, e.g. `import 'some-polyfill'`
          ['^\\u0000'],

          // Node.js builtins
          [`^(${builtinModules.join('|')})(/|$)`],
        ],
      },
    ],
    'spaced-comment': 'error',
    yoda: ['error', 'never'],
  },
  overrides: [
    {
      files: ['**/*.js'],
      rules: {
        'import/no-unresolved': 'error',
        'no-unused-vars': [
          'error',
          {
            argsIgnorePattern: '^_',
            caughtErrors: 'all',
            caughtErrorsIgnorePattern: '^_',
            ignoreRestSiblings: true,
            varsIgnorePattern: '^_',
          },
        ],
        'no-restricted-globals': 'error',
        'import/no-unassigned-import': 'error',
      },
    },
    {
      files: ['**/*.stories.*'],
      rules: {
        'simple-import-sort/exports': 'off',
      },
    },
  ],
}
