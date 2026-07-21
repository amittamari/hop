## 1. Role-aware separator

- [x] 1.1 Expand `thin_rule` signature to accept role, agent, and glyphs; color the entire line in `theme.agent_color(agent)` for `Role::Agent` and keep current gray for `Role::User`
- [x] 1.2 Add agent glyph to the separator label when role is `Agent` and icons are enabled (glyph + space + badge); omit glyph for `User` and when icons are disabled
- [x] 1.3 Update `render_transcript` call site to pass role, agent, and glyphs through to `thin_rule`

## 2. Tests

- [x] 2.1 Unit test: agent separator uses brand color and includes glyph when icons enabled
- [x] 2.2 Unit test: user separator retains current gray styling with no glyph
- [x] 2.3 Unit test: agent prose and code body lines remain undecorated and preserve existing indentation
