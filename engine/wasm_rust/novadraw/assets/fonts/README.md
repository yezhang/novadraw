# Bundled Fonts

Novadraw bundles four complementary font families for deterministic native and Web text:

| Role | Family | File | Upstream |
|---|---|---|---|
| UI and Latin text | Inter | `InterVariable.ttf` | https://github.com/rsms/inter |
| Simplified Chinese and CJK fallback | Noto Sans SC | `NotoSansSC-VF.ttf` | https://github.com/notofonts/noto-cjk |
| Code and technical labels | JetBrains Mono | `JetBrainsMono-Regular.ttf` | https://github.com/JetBrains/JetBrainsMono |
| Arabic shaping and RTL fallback | Noto Sans Arabic | `NotoSansArabic-VF.ttf` | https://github.com/notofonts/arabic |

All four fonts are distributed under the SIL Open Font License 1.1. Their upstream license
texts are stored in `LICENSES/`.

Font binaries are tracked with Git LFS.

Noto Sans Arabic is the unmodified `NotoSansArabic[wdth,wght].ttf` from
[`google/fonts` at `8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5`](https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notosansarabic),
stored under the local filename above. Its license is `LICENSES/NotoSansArabic-OFL.txt`.
Register it alongside the Latin and CJK fonts to render mixed-script TextFlow content
without relying on platform-installed fonts.

## SHA-256

```text
4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf  InterVariable.ttf
d68bafcb48a2707749396aa12bbbd833cb70401f3a9a689fd2902c7e0d295964  NotoSansSC-VF.ttf
e6fd0d7e91550b3ed2b735d4312474362c4716edc4fc0577a0f61ed782d5aed1  JetBrainsMono-Regular.ttf
63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e  NotoSansArabic-VF.ttf
```
