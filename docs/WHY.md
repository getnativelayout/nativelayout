# nativelayout - Why it exists, and how it compares

This page records why nativelayout exists, where it sits among similar tools, and how
to answer the criticisms that come up most often. It is meant to be honest: it also says
where nativelayout is not the right choice.

## 1. Motivation

Many developers have a phone and no laptop. Good tools for that reality are rare, and the
ones that exist (CodeAssist, for example, which made Android development possible on a
phone) prove that the need is real.

nativelayout follows the same idea for user interfaces: a screen is a small file that can
be written, previewed and fixed on a phone, and that still produces a real native UI.

## 2. What it is

nativelayout is a declarative UI toolkit. A screen is written in a `.nl` file (HTML-like
markup plus CSS), the program hands it plain data, and the result is shown as native views.

```
.nl + CSS + data  ->  Expander  ->  layout engine  ->  native tree  ->  differ  ->  host
```

The host is the only platform-specific part: Android views (Kotlin), or a Kivy canvas
(Python). The core (parser, expander, layout, diff) exists in Kotlin and in Python, follows
one Specification, and a Rust core is in progress. Other languages and hosts (C, Swift, and
so on) are meant to come from the Rust core, with one host per platform.

## 3. The right comparison: UI toolkits, not template engines

A template engine (Jinja, Liquid, Mustache, Handlebars) takes a template and data and
produces text. Its work ends there.

nativelayout starts with a template step, but that is only the first stage. What follows is
what makes it a UI toolkit:

- CSS and flex layout resolved into a native tree;
- a differ that updates only what changed between two renders (with `key` for list identity);
- real native widgets, text fields, virtualized lists, layers (dialogs), a fixed footer and dock;
- elastic scrolling, entrance animations, state preserved in fields between renders.

So the useful comparison is with the stack of declarative UI toolkits:

| Tool | Description | Relation to nativelayout |
|---|---|---|
| Flutter | Declarative UI in Dart, draws its own widgets | Same family. Flutter draws everything itself; the Android host of nativelayout uses the platform's own views. |
| Jetpack Compose | Declarative UI in Kotlin for Android | Same family. Compose is code; nativelayout is a file in markup. |
| JSX / React Native | Markup inside JavaScript, mapped to native views | Closest in spirit: markup describing native views, re-rendered and diffed. JSX embeds arbitrary JavaScript; nativelayout deliberately does not (see section 5). |
| SDL | Low-level windows, input and drawing | Lower level. SDL gives a surface and input; there is no layout, text field or list. A toolkit like this sits above something like it. |
| Kivy | Python UI toolkit with its own language (kv) | The Python host of nativelayout runs on Kivy. nativelayout is the layer above: markup, CSS, diff, and the same files as Android. |
| Jinja / Liquid | Text templates | Only the first stage resembles them (see section 4). |

## 4. Inspirations (stated openly)

- Control lines (`if x:`, `for x in list:`, `end`) are inspired by Python and by text
  template languages. They are not Jinja syntax: Jinja uses `{% %}` and `{{ }}`, while
  nativelayout uses standalone control lines and plain `{ }` for values.
- Markup and CSS come from HTML and CSS, so the language is already familiar.
- `key` for list identity comes from React.
- Entrance motion uses the spring and the curves of framer-motion.
- Elastic scroll follows the behavior users already know from mobile platforms.

Using ideas that work is normal. The claim is not that each idea is new, but that the
combination is useful (see section 6).

## 5. Deliberate design choices

- **No code inside markup.** Conditions and values are limited on purpose: no arithmetic,
  only names with dots. The program calculates and hands over the result. This keeps a
  `.nl` file readable by a designer, checkable by a tool, and valid in every language port.
- **One Specification, many cores.** Behavior is defined by the Spec, not by one
  implementation. Kotlin and Python must agree with it, and so will Rust.
- **Plain data.** Data is text, number, bool, list and map, like JSON. Nothing platform
  specific crosses into the file.

## 6. What it offers

- The same `.nl` and CSS files render on Android (Kotlin) and in Python (Kivy).
- Native views on Android, with no WebView.
- Very little boilerplate: a complete app is one short program file plus the `.nl` files.
- It can be written and edited on a phone. The online editor (pad) previews the same
  files in the browser.
- Python on Android through a single `import nativelayout as nl`.

## 7. Where it is not the best choice

- A large production app that needs the whole platform ecosystem today: use Compose,
  Flutter or React Native. They have years of tooling, libraries and community.
- Heavy custom drawing or games: use an engine.
- The Python host inherits the size of Kivy (an APK is about 19 MB). That comes from
  Kivy, not from nativelayout.
- It is young. Known limitations are tracked openly: scrollable regions, loading
  indicators, https images and a larger icon set are the next items.

## 8. Common criticisms

**"It is reinventing the wheel."**
Every toolkit is a wheel that already exists in some form. The question is what the new
one adds. Here: markup in a file with CSS, one Spec shared by several languages, native
views, and a workflow that works from a phone. If that does not matter to someone, other
tools serve them well.

**"It copied Jinja."**
Only the first stage is a template step, and its syntax is not Jinja's (see section 4).
That step is a small part of the project. The rest is a layout engine, a differ and the
hosts.

**"It is just a template engine."**
A template engine produces text. This produces a native view tree, keeps it, and updates it
by diff. That is the definition of a UI toolkit.

**"Why not Compose, Flutter or React Native?"**
For many projects, that is the right choice. nativelayout targets a different case: UI as
files, shared across languages, small and editable anywhere.

**"Nobody will use it."**
Adoption is earned with working apps and good documentation. The repository keeps working
example apps (Despesas) for that reason.

## 9. How to answer criticism

- A concrete technical point ("X is missing", "Y breaks") is useful, even when it stings.
  Record it as an issue. Much of the roadmap came from points like this.
- General skepticism ("why another toolkit?") is normal for any new project. Answer with a
  demonstration, not an argument.
- Provocation without content needs no answer.

The best reply to all three is the same: a working app and a clear page like this one.