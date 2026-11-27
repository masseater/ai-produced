import { repositoryConfig } from "@r_masseater/msch/config";
import { decisionRules, remotionRules, tracedRules } from "@r_masseater/msch/lint";
import { workspaceSettings } from "@r_masseater/msch/workspace-settings";
import { defineConfig } from "vite-plus";

const workspace = await workspaceSettings({
  root: import.meta.dirname,
  ruleSets: { decision: decisionRules, traced: tracedRules, remotion: remotionRules },
  dependencySets: {},
});

export default defineConfig({
  ...repositoryConfig({
    workspace,
    textlint: {
      kanji: [
        "外国電子決済手段",
        "電子決済手段等取引業",
        "前払式支払手段",
        "犯罪収益移転防止法",
        "事業者間取引適正化等法",
        "個人情報保護法",
        "特定商取引法",
        "電気通信利用役務",
        "特定業務委託事業者",
      ],
      rules: { "@r_masseater/preset-ja": { "no-taigendome": false } },
    },
  }),
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
      commit: "ff191de7d903788eceaf516a514ae7d3b56b924a",
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
      commit: "2b4aa13a72dd47cd135b06ce5abfccd92e19b24f",
      root: "skills",
      skills: ["modern-web-guidance"],
    },
    "masseater/skills": {
      commit: "571941a11bcad8418f7857cc5732cb3adf6fabcf",
      root: "plugins/masseater-skills/skills",
      skills: "all",
    },
    "mkosir/typescript-style-guide": {
      commit: "170074a0df9b07f4a4a78e3596ec1edf06a14da7",
      root: "skills",
      skills: ["typescript-discriminated-unions", "typescript-variables"],
    },
    "nanaism/yomiyasu": {
      commit: "3979c750b5e1fe9892bd47bb354f0fb51d534ff2",
      root: "skills",
      skills: ["yomiyasu"],
    },
    "obra/superpowers": {
      commit: "22f0b463e7a5e579904dd6211a520321035526f7",
      root: "skills",
      skills: ["systematic-debugging", "verification-before-completion"],
    },
    "shadcn-ui/ui": {
      commit: "0e5b2e621a9ee4cf6a604bbe6c5c0d5b70561eb4",
      root: "skills",
      skills: ["shadcn"],
    },
    "./docs/skills": { skills: "all" },
  },
  msch: { workspaceAgentsMd: true },
});
