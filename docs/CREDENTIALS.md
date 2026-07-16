# Provider credential contract

Lantern's first live model path delegates authentication to the pinned local
Pi driver. The developer starts Pi directly, runs `/login`, and chooses OpenAI
Codex. Pi owns the resulting subscription credential and its lifecycle;
Lantern does not ask for, parse, copy, persist, refresh, or display it.

This is deliberately not a general credential subsystem. In the current
slice, Lantern has:

- no API-key command, setting, protocol field, or database column;
- no conversion between ChatGPT subscription access and OpenAI API billing;
- no automatic provider or authentication fallback; and
- no credential value in process arguments, prompts, events, or diagnostics.

`LANTERN_PI_BIN`, `LANTERN_PI_MODEL`, and `LANTERN_MODEL_WORKDIR` select the
local driver executable, model, and isolated working directory. They are not
credential inputs. Pi runs as a trusted local dependency under the developer's
account and receives the normal process environment, as locally installed
command-line tools do. It resolves its own authentication state without
returning that state to Lantern.

If Pi rejects a request, Lantern reports a fixed failure and tells the
developer to inspect status in Pi and authenticate there if required.
Arbitrary provider error detail and stderr are excluded because either may
contain sensitive material. Lantern does not retry with another identity,
model, or provider.

Adding a direct provider adapter later requires a separate reviewed decision
covering its credential source, process-environment boundary, OS keychain
support, revocation, and tests. It must not weaken this delegated path or add a
silent fallback.
