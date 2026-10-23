# F6 - Слепая приёмка v4: gitleaks без ложных срабатываний, честные лимиты на dev-mock

**Требования:** G04, G26
**Зависит от:** F5
**Зона:** `.gitleaks.toml` (новый) · `ui/dev-mock.js` и `ui/js/v4/mock-*.js` (только лимиты сред) · `docs/security.md` (строка про gitleaks) · `ui/selftest.js`
**Волна:** 17
**Ревью:** нет
**Модель:** обычная

## Что должно заработать

Слепая приёмка 23.10 (G4 v4):
1. `gitleaks git .` по всей истории - 91 срабатывание `generic-api-key`, все ложные: `api_message_id` в `core/tests/fixtures/claude_stream_partial.jsonl`, минифицированный `ui/vendor/xterm/*.js`, тестовое `tok-123456789` в `core/tests/kits.rs`, i18n-ключ в `ui/js/screens/settings.js`. Условие: `.gitleaks.toml` с `[extend] useDefault = true` и allowlist по путям и regex ровно для этих мест (не глушить правило целиком); `gitleaks git . --no-banner` и `gitleaks dir . --no-banner` → 0 находок; строка в `docs/security.md` - «gitleaks clean, allowlist в .gitleaks.toml, что в нём и почему».
2. dev-mock рисует у Gemini «День 85%», а README говорит, что реальные лимиты есть только у Claude Code. Условие: на dev-mock у сред без данных о лимитах - «лимиты: нет данных», как в настоящем ядре; selftest на это.

## Критерии приёмки

1. `gitleaks git . --no-banner` и `gitleaks dir . --no-banner` - «no leaks found».
2. selftest: dev-mock у Gemini/Codex показывает «нет данных», у Claude Code - проценты.
3. `node scripts/ui-selftest.js` и `cargo test --workspace` зелёные.
