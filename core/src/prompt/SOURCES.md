# Sources of the prompt templates

Every template in `render.rs` follows the official prompting guide of the model's vendor. Each link
below was opened on 2026-10-21 (through OpenCLI, in Chrome) and led to a page of the vendor itself.
A rule that was not found on those pages is not in the templates. Each rule has an id used in code
comments, and every rule that changes the shape of a prompt has a test in `core/tests/engine_prompt.rs`.

## Anthropic (Claude)

- Prompting best practices, https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices, opened 2026-10-21.
  - **A1** "Provide instructions as sequential steps using numbered lists or bullet points when the order or completeness of steps matters." The "How to work" part of every prompt is a numbered list.
  - **A3** "XML tags help Claude parse complex prompts unambiguously ... Use consistent, descriptive tag names." The Anthropic layout wraps each kind of content in its own tag: `request`, `files`, `diff`, `goal`, `constraints`, `done_when`, `plan`, `steps`.
  - **A4** "Setting a role in the system prompt focuses Claude's behavior." The role sentence is the first sentence of the Anthropic system prompt, and the task prompt has no role.
  - **A5** "Put longform data at the top ... above your query, instructions, and examples." Data blocks (the person's words, files, diff, check output) come before the instruction tags.
  - **A9** The sample prompt in "Avoid focusing on passing tests and hardcoding" ("Implement a solution that works correctly for all valid inputs, not just the test cases. Do not hard-code values ...") is the last step for the executor when the project has tests.
- Prompting Claude Sonnet 5.5, https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-sonnet-5-5, opened 2026-10-21.
  - **A10** "Parse the last JSON value in the response ... Don't take everything from the first `{` to the last `}`. The model occasionally writes a draft before its final JSON." `prompt::last_json_object` does exactly that for every structured answer.

Not taken: the model-specific sections of other Claude models, the "overeagerness" prompt (it names Opus 4.5 and 4.6 only), the examples and thinking sections (templates carry no examples and set no thinking).

## OpenAI (GPT, Codex)

- Prompt engineering, https://platform.openai.com/docs/guides/prompt-engineering (the page now lives at https://developers.openai.com/api/docs/guides/prompt-engineering), opened 2026-10-21.
  - **O1** "Markdown headers and lists can be helpful to mark distinct sections of a prompt ... XML tags can help delineate where one piece of content begins and ends"; a developer message holds Identity, Instructions, Examples, Context "usually in this order". The system prompt is `# Identity` then `# Instructions`; the task prompt is `# Instructions` then `# Context`.
  - **O2** Context "is usually best positioned near the end of your prompt". All data blocks are under `# Context`, the last part.
- Codex Prompting Guide, https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide (opened as https://cookbook.openai.com/examples/gpt-5/codex_prompting_guide), opened 2026-10-21.
  - **O3** "Autonomy and Persistence": "Persist until the task is fully handled end-to-end ... Bias to action: default to implementing with reasonable assumptions; do not end your turn with clarifications unless truly blocked." In the executor's system prompt.
  - **O4** "You should also remove all prompting for the model to communicate an upfront plan, preambles, or other status updates during the rollout." The OpenAI templates contain none (checked by test).
  - **O5** "Editing constraints": "NEVER revert existing changes you did not make." In the executor's system prompt.

Not taken: the starter prompt's tool rules (`rg`, `apply_patch`, parallel reads), they describe Codex's own harness and tools, which the app does not control.

## Google (Gemini)

- Prompt design strategies, https://ai.google.dev/gemini-api/docs/prompting-strategies (section "Gemini 3"), opened 2026-10-21.
  - **G1** "Be precise and direct ... Avoid unnecessary or overly persuasive language." No flattery or emphasis in the Google templates.
  - **G2** "Use consistent structure ... XML-style tags or Markdown headings are effective. Choose one format and use it consistently within a single prompt." Google templates use XML only.
  - **G3** "Place essential behavioral constraints, role definitions (persona), and output format requirements in the System Instruction or at the very beginning of the user prompt." The system prompt is `role`, `constraints`, `output_format`; the task prompt opens with `constraints`.
  - **G4** "Supply all the context first. Place your specific instructions or questions at the very end of the prompt" and "use a clear transition phrase ... 'Based on the information above...'". Order: `constraints`, `context`, `task`; the task ends with that phrase.

Not taken: the Gemini 3 Flash clauses (current day, knowledge cutoff, grounding), the agentic "system instruction template" (long, evaluated for rule-book agents, not for coding), "Think step-by-step".

## Alibaba (Qwen)

- Alibaba Cloud Model Studio: Text-to-text prompt guide, https://www.alibabacloud.com/help/en/model-studio/prompt-engineering-guide, opened 2026-10-21. This is the guide of the Qwen API platform; Alibaba has no guide written for Qwen Code (see CONCERNS in the task report).
  - **Q1** The prompt framework: Context, Objective, ..., Response named apart. `#Context#`, `#Objective#`, `#Constraints#`, `#Task Steps#`, `#Response#`.
  - **Q2** "Set steps for tasks" (`#Task Steps#`).
  - **Q3** "Use separators to distinguish units ... unique character combinations that are rare in natural language text, such as: `###`, `===`, `>>>`." Units are separated by `###` lines.

## opencode, Cursor, own commands

No official prompting guide was found for opencode (https://opencode.ai/docs/ describes the tool, not prompting; the pages found by search were third-party). Cursor and own commands have none either. These envs list models of many vendors, so the template comes from the vendor of the model that is really started (`Vendor::from_name` on the catalog's family, then `Vendor::from_model_id` on the identifier); a model of an unknown vendor gets the plain "Goal / Constraints / Done when" template.
