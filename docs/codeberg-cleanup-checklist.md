# Codeberg Cleanup & Package Verification Checklist

Follow this when you are notified that the AUR package has been updated to
v0.1.2. Do **not** delete the Codeberg repositories until Phase 7.

State verified 2026-10-25:

| Channel | Status today |
|---|---|
| GitHub repo (`jbrains/textifyopendocument`) | OK — tags v0.1.0/0.1.1/0.1.2 pushed, `main` in sync |
| crates.io | OK — 0.1.2 is default, GitHub URLs in metadata |
| AUR | **PENDING** — live package is 0.1.1-1 (maintainer `typebrook`), URL still Codeberg, source tarball still Codeberg |
| Homebrew | **BROKEN** — formula sha256 does not match the GitHub tarball (see Phase 1) |
| Repo `packaging/aur` | **BROKEN** — `b2sums` are 0.1.1's values; `.SRCINFO` stale (0.1.1 + Codeberg URL) |
| Repo `packaging/nix/package.nix` | **BROKEN** — `fetchFromGithub` is not a real nixpkgs fetcher (see Phase 1) |
| Codeberg | Both repos still exist: `textifyopendocument` and `homebrew-tap` |

---

## Phase 1 — Fix packaging defects in the repo (do before or at notification time)

These are independent of the AUR notice and currently block Homebrew installs
and Arch builds.

### 1.1 Homebrew formula checksum (wrong today)

`packaging/homebrew/textifyopendocument.rb` claims
`d5558cd4…`, but the GitHub tarball for tag v0.1.2 hashes to:

```
ea4a6070f47ce155330534de04155cb22fb6a7f1c9c2fc3de20e957e21ed1ca1
```

Recompute to confirm, then update the formula:

```bash
curl -sL https://github.com/jbrains/textifyopendocument/archive/refs/tags/v0.1.2.tar.gz | sha256sum
```

Apply the same change to the live tap repo
(`jbrains/homebrew-tap`, file `Formula/textifyopendocument.rb`) and push.

### 1.2 PKGBUILD b2sums (wrong today)

`packaging/aur/PKGBUILD` still carries 0.1.1's `b2sums`
(`10b627f1…`). The 0.1.2 crate from static.crates.io hashes to:

```
cfced46c601e8897aa01f128028019c0cac4e8c9d58084ce90a7682e37687d72c485cd208758110a130b15e64f9651b6caf0876f4df21bca4d115e89902968b8
```

Recompute to confirm:

```bash
curl -sL https://static.crates.io/crates/textifyopendocument/textifyopendocument-0.1.2.crate | b2sum
```

Update `b2sums` in the PKGBUILD.

### 1.3 Regenerate `.SRCINFO`

Only after 1.2. On an Arch system (or in the Docker container from Phase 2):

```bash
cd packaging/aur
makepkg --printsrcinfo > .SRCINFO
```

Check the result has `pkgver = 0.1.2` and
`url = https://github.com/jbrains/textifyopendocument`.

### 1.4 Fix the Nix fetcher

`packaging/nix/package.nix` line 1 and the `realSrc` block: nixpkgs has no
`fetchFromGithub` and `fetchFromGitHub` takes no `domain` argument. It should
be:

```nix
{ lib, rustPlatform, fetchFromGitHub, src ? null }:
# …
realSrc = if src == null then fetchFromGitHub {
  owner = "jbrains";
  repo = "textifyopendocument";
  rev = "v${version}";
  hash = lib.fakeHash;
} else src;
```

(Only affects the future nixpkgs-PR path; the flake itself passes `src`.)

### 1.5 Optional README touch-ups

- AUR section still says "AUR submission pending". Once the package is live,
  point it at https://aur.archlinux.org/packages/textifyopendocument (e.g.
  `paru -S textifyopendocument`).
- `nix run git+https://github.com/jbrains/textifyopendocument` works; the
  shorthand `nix run github:jbrains/textifyopendocument` is nicer.

### 1.6 Commit and push

```bash
git add packaging docs
git commit -m "fix: correct packaging checksums and nix fetcher after migration"
git push publish main
```

---

## Phase 2 — Verify the AUR package (when notified)

1. Check status:

   ```bash
   curl -s "https://aur.archlinux.org/rpc/v5/info?arg[]=textifyopendocument"
   ```

   Expect: `"Version":"0.1.2-1"` (or newer), `"URL":"https://github.com/jbrains/textifyopendocument"`,
   `"OutOfDate":null`.

2. Inspect the live PKGBUILD:

   ```bash
   curl -sL https://aur.archlinux.org/cgit/aur.git/snapshot/textifyopendocument.tar.gz \
     | tar -xzO textifyopendocument/PKGBUILD
   ```

   Critical checks:
   - `url=` points to GitHub (not Codeberg).
   - `source=` does **not** reference codeberg.org. (The current 0.1.1-1
     PKGBUILD downloads from `$url/archive/v$pkgver.tar.gz` — a Codeberg URL.
     A v0.1.2 PKGBUILD that still does this breaks the moment you delete the
     Codeberg repo.)
   - `b2sums` matches the actual source file (download it and run `b2sum`).

3. Build test (optional but recommended; no Arch machine needed):

   ```bash
   docker run --rm -it archlinux bash -c '
     pacman -Syu --noconfirm base-devel rust git sudo
     useradd -m builder && echo "builder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers
     su builder -c "cd /home/builder &&
       git clone https://aur.archlinux.org/textifyopendocument.git &&
       cd textifyopendocument && makepkg -si --noconfirm"
   '
   ```

   (`makepkg` refuses to run as root, hence the `builder` user.)

4. If the AUR PKGBUILD still references Codeberg or has stale checksums:
   comment on the package page asking the maintainer (`typebrook`) to switch
   the source to the crates.io tarball or the GitHub archive, and **hold off
   on deleting Codeberg** until it is fixed. If you want to maintain the
   package yourself: ask for co-maintainership in the comments; if there is
   no response for ~2 weeks, flag it out-of-date, then file an orphan request.

---

## Phase 3 — Verify Homebrew

1. Check the live tap formula:

   ```bash
   curl -sL https://raw.githubusercontent.com/jbrains/homebrew-tap/main/Formula/textifyopendocument.rb
   ```

   Expect `url` on github.com and `sha256 "ea4a6070…"` (value from Phase 1.1).

2. Recompute the tarball hash (Phase 1.1 command) and confirm equality.

3. Fresh-install test on a machine with Homebrew (macOS or Linux):

   ```bash
   brew untap jbrains/homebrew-tap 2>/dev/null
   brew tap jbrains/homebrew-tap https://github.com/jbrains/homebrew-tap
   brew install textifyopendocument
   textifyopendocument --version   # expect 0.1.2
   brew test textifyopendocument
   ```

4. Note: anyone who tapped the old Codeberg tap will get errors from
   `brew update` after Codeberg is deleted. They must `brew untap` and retap
   from the GitHub URL. Nothing you can do for them beyond having the GitHub
   tap ready (which it already is).

---

## Phase 4 — Verify crates.io

Verified 2026-10-25, re-check cheaply at notification time:

- https://crates.io/crates/textifyopendocument — default version 0.1.2,
  `repository`/`homepage`/`documentation` on github.com. ✓
- Clean install test:

  ```bash
  cd "$(mktemp -d)"
  cargo install textifyopendocument
  textifyopendocument --version   # expect 0.1.2
  ```

- Ownership: `cargo owner --list textifyopendocument` — expect `jbrains`.

Expected blemish, not a bug: the 0.1.0 and 0.1.1 metadata on crates.io still
links to Codeberg (published versions are immutable). Those links will 404
after deletion. 0.1.0 is already yanked; consider yanking 0.1.1 is *not*
necessary — leave it.

---

## Phase 5 — Verify Nix

- Flake (needs Nix installed):

  ```bash
  nix run github:jbrains/textifyopendocument -- --version   # expect 0.1.2
  ```

  (The README's `git+https://github.com/…` form is equivalent.)

- Confirm Phase 1.4's `fetchFromGitHub` fix is committed.

---

## Phase 6 — Final GitHub repo check (right before deleting Codeberg)

```bash
git ls-remote --tags publish          # v0.1.0, v0.1.1, v0.1.2 all present
git fetch publish && git status -sb   # main in sync with publish/main

cd "$(mktemp -d)"
git clone git@github.com:jbrains/textifyopendocument.git check
cd check
cargo build --release
./target/release/textifyopendocument --version   # expect 0.1.2
git log --oneline | wc -l                         # same count as your local repo
```

---

## Phase 7 — Delete the Codeberg repositories (last!)

Only after Phases 2–6 pass, especially Phase 2's "no codeberg.org in the AUR
PKGBUILD" check.

1. Preserve anything worth keeping first: check both Codeberg repos for
   issues, PRs, releases, and wiki pages (`textifyopendocument` **and**
   `homebrew-tap`). Download/copy anything you want to keep.
2. Delete `jbrains/textifyopendocument`:
   repo → Settings → Danger Zone (bottom) → "Delete repository" → type the
   repo name to confirm.
3. Delete `jbrains/homebrew-tap` the same way.
4. There are no redirects after deletion; any remaining Codeberg URL 404s.
5. Optional local cleanup:

   ```bash
   git remote -v                          # no codeberg URLs (publish → GitHub already)
   git config --get-regexp codeberg       # expect no output
   ```

   Optionally remove Codeberg entries from `~/.ssh/config` and
   `~/.ssh/known_hosts`. Keeping your Codeberg *account* for contributing to
   other projects is fine — the ToS issue concerns hosted content, not the
   account itself.

---

## Phase 8 — Post-cleanup smoke test

```bash
grep -ri codeberg . --exclude-dir=.git --exclude-dir=target
```

Expect no hits (after Phase 1, the stale `.SRCINFO` is gone). If anything
remains, evaluate whether it is a user-facing URL and fix it.

Then confirm all four channels from a fresh-user perspective:

- `cargo install textifyopendocument` → 0.1.2
- `paru -S textifyopendocument` (or `makepkg` from the AUR) → 0.1.2
- `brew tap jbrains/homebrew-tap https://github.com/jbrains/homebrew-tap && brew install textifyopendocument` → 0.1.2
- `nix run github:jbrains/textifyopendocument -- --version` → 0.1.2
