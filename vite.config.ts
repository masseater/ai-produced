import htmlReact from "@html-eslint/eslint-plugin-react";
import tanstackQuery from "@r_masseater/oxlint-typed-rules/tanstack-query";
import tanstackRouter from "@r_masseater/oxlint-typed-rules/tanstack-router";
import fsd from "@r_masseater/vp-tools/fsd";
import { plugin as shadcn } from "@shadcn/lint";
import { Struct } from "effect";
import baselineJs from "eslint-plugin-baseline-js";
import { defineConfig } from "vite-plus";

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

export default defineConfig({
  commitlint: { extends: ["@commitlint/config-conventional"] },
  steiger: fsd.configs.recommended,
  test: { projects: ["apps/*", "libs/*", "infra/*", "tools/*"], passWithNoTests: true },
  staged: {
    "*": "vp check --fix",
  },
  fmt: {
    ignorePatterns: generated,
    sortImports: true,
    sortTailwindcss: true,
    sortPackageJson: true,
  },
  lint: {
    ignorePatterns: generated,
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
        files: ["apps/video/**"],
        rules: remotionRules,
      },
      {
        files: ["apps/video/src/app/root.tsx"],
        rules: { "import/no-unassigned-import": ["error", { allow: ["**/*.css"] }] },
      },
      {
        files: ["**/shared/ui/**", "libs/ui/src/components/**"],
        rules: { "shadcn/no-restyle": "off", "react/jsx-props-no-spreading": "off" },
      },
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
          env: ["OPENROUTER_API_KEY"],
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
          "markdown-lint",
          "renovate",
          "actions-lint",
          "steiger",
          "fallow",
          "audit",
        ],
      },
      test: "vp test run",
      syncpack: "syncpack lint",
      "markdown-lint": [
        'textlint "**/*.md"',
        `git -c core.quotePath=false ls-files -co --exclude-standard '*.md' | git -c core.quotePath=false -c core.excludesFile=.textlintignore check-ignore --no-index --stdin -nv | awk -F '\\t' '$1 == "::" { print $2 }' | tr '\\n' '\\0' | xargs -0 -r -n 1 -P 4 python3 "$(skills ls --json | jq -r '.[] | select(.name == "yomiyasu").path')/scripts/yomiyasu_lint.py" --strict`,
      ],
      steiger: "vp-tools steiger apps/*/src",
      audit: {
        command:
          'before="$(cksum pnpm-workspace.yaml)"; vp pm audit --fix || exit; if [ "$before" != "$(cksum pnpm-workspace.yaml)" ]; then echo "vp pm audit --fix rewrote pnpm-workspace.yaml. Run vp install and commit the diff." >&2; exit 1; fi',
        cache: false,
      },
      betterleaks: {
        command:
          'go run github.com/betterleaks/betterleaks/v2@v2.0.0-rc.7 git . --config .betterleaks.toml --redact --no-banner --revision="$(git rev-parse -q --verify "$BETTERLEAKS_BASE" || echo origin/main)..HEAD"',
        cache: false,
      },
      "betterleaks:staged": {
        command:
          "git diff --cached --no-ext-diff --no-textconv --no-color --src-prefix=a/ --dst-prefix=b/ | go run github.com/betterleaks/betterleaks/v2@v2.0.0-rc.7 stdin --git-diff --config .betterleaks.toml --redact --no-banner",
        cache: false,
      },
      renovate: "renovate-config-validator --strict",
      fallow: [
        "fallow --production --quiet --format compact",
        "fallow dead-code --no-production --quiet --format compact",
        "fallow dupes --quiet --format compact",
        "fallow security --fail-on-issues --quiet",
      ],
      "actions-lint": {
        command:
          "uvx --from actionlint-py@1.7.12.25 actionlint && uvx zizmor@1.32.0 .github && ast-grep scan --rule .github/rules/no-full-history-checkout.yml .github/workflows",
        cache: {
          untrackedEnv: [
            "GH_TOKEN",
            "ZIZMOR_NO_ONLINE_AUDITS",
            "HTTPS_PROXY",
            "https_proxy",
            "NO_PROXY",
            "no_proxy",
            "SSL_CERT_FILE",
          ],
        },
      },
    },
  },
});
