import htmlReact from "@html-eslint/eslint-plugin-react";
import { workspaceSettings } from "@r_masseater/msch/workspace-settings";
import tanstackQuery from "@r_masseater/oxlint-typed-rules/tanstack-query";
import tanstackRouter from "@r_masseater/oxlint-typed-rules/tanstack-router";
import { plugin as shadcn } from "@shadcn/lint";
import { Struct } from "effect";
import baselineJs from "eslint-plugin-baseline-js";
import { defineConfig } from "vite-plus";

import { checkTasks, checks } from "./checks.config.ts";
import {
  allRulesOf,
  asErrors,
  effectRules,
  eslintReactRules,
  generated,
  reactHooksRules,
  remotionRules,
  restrictedImports,
} from "./lint.config.ts";

const typedRules = "@r_masseater/oxlint-typed-rules";

const workspace = await workspaceSettings({
  root: import.meta.dirname,
  ruleSets: { remotion: remotionRules },
  dependencySets: {},
});

export default defineConfig({
  ...checks,
  skills: {
    "better-auth/skills": {
      commit: "20c9e88a5c007461a703f1c213572b073196113e",
      skills: [
        "better-auth-best-practices",
        "better-auth-security-best-practices",
        "email-and-password-best-practices",
      ],
    },
    "cloudflare/skills": {
      commit: "cbf893b954222276bf1af57e5f23379231e0efb9",
      root: "skills",
      skills: ["cloudflare", "workers-best-practices"],
    },
    "Effect-TS/skills": {
      commit: "155c50c911f9c99c294ef8a4461981c5bbe6453e",
      root: "skills",
      skills: ["effect-ts"],
    },
    "elysiajs/skills": {
      commit: "1466d85aa4f2a0fec91f71311e65bbfa91aa7dc5",
      skills: ["elysiajs"],
    },
    "feature-sliced/skills": {
      commit: "5e95aaebb84541742686e56352b7569acf4306f1",
      skills: ["feature-sliced-design"],
    },
    "GoogleChrome/modern-web-guidance": {
      commit: "4dd968349e54ca45a05dd5c818fb035c9054b022",
      root: "skills",
      skills: ["modern-web-guidance"],
    },
    "masseater/skills": {
      commit: "b323dbad64cdb7140b75686cb129a36b670a621a",
      root: "plugins/masseater-skills/skills",
      skills: "all",
    },
    "mkosir/typescript-style-guide": {
      commit: "170074a0df9b07f4a4a78e3596ec1edf06a14da7",
      root: "skills",
      skills: ["typescript-discriminated-unions", "typescript-variables"],
    },
    "nanaism/yomiyasu": {
      commit: "db78b9bc5e868549b517a037df7a4d1c93edac23",
      root: "skills",
      skills: ["yomiyasu"],
    },
    "obra/superpowers": {
      commit: "22f0b463e7a5e579904dd6211a520321035526f7",
      root: "skills",
      skills: ["systematic-debugging", "verification-before-completion"],
    },
    "shadcn-ui/ui": {
      commit: "e62c9435fff729a175f38983dee73b2cb4f6a069",
      root: "skills",
      skills: ["shadcn"],
    },
    "./docs/skills": { skills: "all" },
  },
  test: { projects: [...workspace.testProjects], passWithNoTests: true },
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    ignorePatterns: [...generated, ...workspace.lint.ignorePatterns],
    sortImports: true,
    sortTailwindcss: true,
    sortPackageJson: true,
  },
  lint: {
    ignorePatterns: [...generated, ...workspace.lint.ignorePatterns],
    plugins: [
      "eslint",
      "typescript",
      "unicorn",
      "oxc",
      "import",
      "react",
      "react-perf",
      "jsx-a11y",
      "jsdoc",
      "promise",
      "node",
      "effecttsgo",
    ],
    categories: {
      correctness: "error",
      suspicious: "error",
      pedantic: "error",
      perf: "error",
      style: "error",
      restriction: "error",
      nursery: "error",
    },
    jsPlugins: [
      { name: "vite-plus", specifier: "vite-plus/oxlint-plugin" },
      { name: "tanstack-query", specifier: `${typedRules}/tanstack-query` },
      { name: "tanstack-router", specifier: `${typedRules}/tanstack-router` },
      { name: "tanstack-start", specifier: `${typedRules}/tanstack-start` },
      { name: "drizzle", specifier: `${typedRules}/drizzle` },
      { name: "react-hooks-js", specifier: "eslint-plugin-react-hooks" },
      { name: "@eslint-react", specifier: `${typedRules}/react` },
      { name: "shadcn", specifier: "@shadcn/lint" },
      { name: "no-comments", specifier: "eslint-plugin-no-comments" },
      { name: "baseline-js", specifier: "eslint-plugin-baseline-js" },
      { name: "@html-eslint/react", specifier: "@html-eslint/eslint-plugin-react" },
      { name: "@remotion", specifier: `${typedRules}/remotion` },
    ],
    rules: {
      "vite-plus/prefer-vite-plus-imports": "error",
      "tanstack-start/no-async-client-component": "error",
      "tanstack-start/no-client-code-in-server-component": "error",
      "no-comments/disallowComments": "error",
      ...allRulesOf({ prefix: "tanstack-query", plugin: tanstackQuery }),
      ...allRulesOf({ prefix: "tanstack-router", plugin: tanstackRouter }),
      "drizzle/enforce-delete-with-where": "error",
      "drizzle/enforce-update-with-where": "error",
      ...allRulesOf({ prefix: "shadcn", plugin: shadcn }),
      ...allRulesOf({ prefix: "baseline-js", plugin: baselineJs }),
      ...baselineJs.configs.recommended().rules,
      ...asErrors(Struct.keys(htmlReact.configs.all.rules ?? {})),
      ...reactHooksRules,
      ...eslintReactRules,
      ...effectRules,
      "eslint/sort-imports": "off",
      "eslint/sort-keys": "off",
      "import/no-named-export": "off",
      "import/prefer-default-export": "off",
      "react/react-in-jsx-scope": "off",
      "eslint/no-undef": "off",
      "typescript/promise-function-async": "off",
      "react/forbid-component-props": "off",
      "oxc/no-rest-spread-properties": "off",
      "node/no-top-level-await": "off",
      "eslint/one-var": ["error", "never"],
      "eslint/no-restricted-imports": ["error", restrictedImports],
      "typescript/prefer-readonly-parameter-types": [
        "error",
        {
          ignoreInferredTypes: true,
          allow: [
            { from: "lib", name: "Request" },
            { from: "package", package: "@tanstack/query-core", name: "QueryClient" },
            {
              from: "package",
              package: "react",
              name: ["ReactNode", "ButtonHTMLAttributes", "ClassAttributes"],
            },
            { from: "package", package: "clsx", name: "ClassValue" },
            {
              from: "package",
              package: "effect",
              name: ["Cause", "Context", "Effect", "Exit", "Layer", "Option"],
            },
          ],
        },
      ],
      "eslint/no-duplicate-imports": ["error", { allowSeparateTypeImports: true }],
      "react/function-component-definition": [
        "error",
        { namedComponents: "arrow-function", unnamedComponents: "arrow-function" },
      ],
      "eslint/new-cap": [
        "error",
        {
          capIsNewExceptions: ["CloudflareApiLive", "Stack"],
          capIsNewExceptionPattern: String.raw`^(Config|Context|Data|Schema|D1|Website|Workers)\.`,
        },
      ],
      "typescript/no-empty-interface": ["error", { allowSingleExtends: true }],
      "typescript/no-empty-object-type": ["error", { allowInterfaces: "with-single-extends" }],
      "react/jsx-filename-extension": ["error", { extensions: [".tsx"] }],
      "react/only-export-components": ["error", { allowExportNames: ["Route"] }],
    },
    overrides: [
      {
        files: ["**/*.config.ts", "**/alchemy.run.ts"],
        rules: { "import/no-default-export": "off" },
      },
      {
        files: ["vite.config.ts"],
        rules: {
          "baseline-js/use-baseline": ["error", { ignoreFeatures: ["top-level-await"] }],
        },
      },
      {
        files: ["**/shared/ui/**"],
        rules: { "shadcn/no-restyle": "off", "react/jsx-props-no-spreading": "off" },
      },
      ...workspace.lint.overrides,
    ],
    options: {
      typeAware: true,
      typeCheck: true,
      denyWarnings: true,
      reportUnusedDisableDirectives: "error",
    },
  },
  run: {
    cache: true,
    tasks: {
      verify: {
        command: "vp check --deny-warnings",
        cache: {
          untrackedEnv: [
            "CLAUDECODE",
            "CLAUDE_CODE_REMOTE",
            "HTTPS_PROXY",
            "https_proxy",
            "NO_PROXY",
            "no_proxy",
            "SSL_CERT_FILE",
            "NODE_EXTRA_CA_CERTS",
          ],
        },
        dependsOn: [
          "test",
          "syncpack",
          "vpa",
          "renovate",
          "actions-lint",
          "fallow",
          "audit",
          "msch",
          ...workspace.tasks.verify,
        ],
      },
      test: "vp test run",
      syncpack: "syncpack lint",
      ...checkTasks,
      audit: {
        command:
          'before="$(cksum pnpm-workspace.yaml)"; vp pm audit --fix || exit; if [ "$before" != "$(cksum pnpm-workspace.yaml)" ]; then echo "vp pm audit --fix rewrote pnpm-workspace.yaml. Run vp install and commit the diff." >&2; exit 1; fi',
        cache: false,
      },
      renovate: "renovate-config-validator --strict",
      fallow: [
        "fallow --production --quiet --format compact",
        "fallow dead-code --no-production --quiet --format compact",
        "fallow dupes --quiet --format compact",
        "fallow security --fail-on-issues --quiet",
      ],
      "actions-lint":
        "ast-grep scan --rule .github/rules/no-full-history-checkout.yml .github/workflows",
    },
  },
});
