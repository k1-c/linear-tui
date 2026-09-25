# Signing in

Run `linear-tui` and, the first time, it asks how you want to connect. Nothing
has to be set up beforehand. Everything below can also be done from the command
line.

## Through the browser

```sh
linear-tui auth login
```

This opens Linear in your browser, and the authorization screen appears as
**k1-c/tui** — the application linear-tui is registered as. (Linear does not
allow "Linear" in an application's name, which is why it is not called
linear-tui there.)

The authorization screen is shown every time, even after you have approved the
application once. It names the workspace the token will be issued for; if you
belong to several, switch to the one you mean there before you approve. Once
you are back, linear-tui prints who you signed in as and in which workspace.

Approving it hands a token back to a local callback on port 53681, 53682, or
53683, whichever is free. The token is stored in
`~/.config/linear-tui/tokens.json`, readable only by you, and refreshed
automatically.

No client secret is involved: Linear's PKCE flow makes one optional, so
linear-tui ships as a public OAuth client.

## With a personal API key

Useful when the browser cannot reach your terminal — over SSH, for example,
where the callback would land on the wrong machine.

Create a key under
[Settings > Account > Security & access](https://linear.app/settings/account/security),
then:

```sh
linear-tui auth token <your-api-key>
```

The key is verified before it is saved, to `config.toml` (`[auth] api_key`).
When both are present, the OAuth token is used.

## Checking and clearing credentials

```sh
linear-tui auth status        # which credentials are in use, and who they belong to
linear-tui auth logout        # forget the OAuth token
linear-tui auth logout --all  # forget the API key in config.toml as well
```

## Using your own Linear application

Some workspaces require third-party applications to be approved by an admin. If
that blocks you — or you would simply rather authorize against your own —
register one at [Linear Settings > API](https://linear.app/settings/api) with
`http://localhost:53681/callback` (plus 53682 and 53683) as its redirect URIs,
then:

```sh
linear-tui auth set-oauth <client-id> [client-secret]
linear-tui auth login
```

`LINEAR_CLIENT_ID` and `LINEAR_CLIENT_SECRET` in the environment override what
`config.toml` says.

## Where credentials are kept

| File | Holds |
| --- | --- |
| `~/.config/linear-tui/tokens.json` | The OAuth access and refresh tokens |
| `~/.config/linear-tui/config.toml` | An API key or your own application's client id and secret, if you set them |

Both are written owner-only (`0600`), atomically. The headless commands for
agents ([cli.md](cli.md)) use the same credentials and never prompt.

On macOS the directory is `~/Library/Application Support/linear-tui`;
`linear-tui paths` prints the directories on any platform.
