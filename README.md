<div align="center">

<img src="img/logo.jpg" alt="nativelayout" width="140">

<h1>nativelayout</h1>

<p><strong>Create native interfaces from HTML, in any language.</strong></p>

<p><a href="https://getnativelayout.github.io/nativelayout/"><strong>Website</strong></a> &middot; <a href="SPEC.md">Specification</a> &middot; <a href="android/README.md">Android guide</a> &middot; <a href="https://github.com/getnativelayout/nativelayout/releases">Releases</a></p>

<a href="https://jitpack.io/#getnativelayout/nativelayout"><img src="https://jitpack.io/v/getnativelayout/nativelayout.svg" alt="JitPack"></a>
<a href="LICENSE"><img src="https://img.shields.io/badge/license-MPL%202.0-blue.svg" alt="License: MPL 2.0"></a>
<a href="https://doi.org/10.5281/zenodo.23470439"><img src="https://zenodo.org/badge/DOI/10.5281/zenodo.23470439.svg" alt="DOI"></a>
<br>
<a href="https://discord.gg/638USuPnpk"><img src="https://img.shields.io/badge/-Join%20our%20Community%20on%20Discord-5865F2?logo=discord&logoColor=white&style=for-the-badge" alt="Join our Community on Discord"></a>

</div>

<br>

<p align="justify">
A screen is written once, in a plain text file that looks like HTML. The same file works in any language: each language only supplies the Expander, the Engine and the native host. No WebView, no canvas: the file becomes real native views.
</p>

<p align="justify">
The file describes the screen. The logic (state, taps) stays in your language, in a screen function.
</p>

## Article

<p align="justify">
The motivation, the proposal and the first results on long lists are described in the technical article:
</p>

> Rafael, A. C. (2026). *Describing Native Android Interfaces with HTML and CSS: A Proposal for Simplification*. Zenodo. [https://doi.org/10.5281/zenodo.23470439](https://doi.org/10.5281/zenodo.23470439)

<p align="justify">
To cite this project, use the "Cite this repository" button on GitHub (see <a href="CITATION.cff"><code>CITATION.cff</code></a>).
</p>

## Community

<p align="justify">
<a href="https://discord.gg/638USuPnpk">Join our Community on Discord</a> to ask questions, share ideas and show the screens you built. Bugs and feature proposals go in the <a href="../../issues">issue tracker</a>, so they are not lost.
</p>

## Why nativelayout

- **HTML you already know**, plus three additions: `{value}`, `if` / `for` lines, and a `<style>` with simple selectors.
- **Native output.** Buttons, fields, lists and images are real platform views.
- **No functions, classes or variables in the file.** State and taps live in your code.
- **Conditions and repetition**, so it is more than a static canvas.
- **Efficient updates.** Each `render` is diffed against the previous screen and only the changes are applied. Text typed in a field is kept.
- **Navigation and animation.** `go` and `goBack` move between screens, the system back button reaches the program, and screens and dialogs fade in by default.
- **Components.** `<include src="card.nl" name="{p.name}">` reuses a `.nl` file with parameters, in a screen or in each item of a list.
- **Identity inside lists.** The same `id` repeats in every item; `clicked("p")` gives the tapped item and `value("qty", p)` reads the field of one item. Each item is found by its key, so what the user typed follows the item.
- **Fixed header and built-in icons.** `<header>` stays at the top while the screen scrolls, and `<img src="icon:arrow-left">` draws a built-in icon in the text color, with no image files.
- **Clear errors** with the line of the `.nl` file.

## Implementations

<p align="justify">
The <code>.nl</code> file is the same everywhere. Each language only needs an Expander, an Engine and a native host. Every implementation lives in its own folder, is published as a library, and has its own guide and examples. The Android folder also contains an editor app (<code>android/app</code>) with live preview, used to test screens on-device.
</p>

| Language | Target | Status | Folder |
|---|---|---|---|
| Kotlin | Android (native views) | implemented | [`android/`](android/) |
| Java | Android (native views) | planned | - |
| Python | Kivy | in progress | [`python/`](python/) |
| Python | Tkinter | in progress | [`python/`](python/) |
| Swift | iOS (UIKit) | planned | - |
| JavaScript / TypeScript | React Native | planned | - |
| Dart | Flutter | planned | - |
| C# | .NET MAUI | planned | - |
| Java | Swing / JavaFX (desktop) | planned | - |
| C++ | Qt | planned | - |
| Rust | desktop | planned | - |
| Go | desktop | planned | - |

<p align="justify">
Want to add a language? See <a href="#contributing">Contributing</a>.
</p>

## Try it

<p align="center"><a href="https://nativelayoutpad.vercel.app/"><strong>Open the online editor</strong></a></p>

<p align="center">
  <a href="https://nativelayoutpad.vercel.app/">
    <img src="img/editor.jpg" alt="nativelayout online editor: code, data and live preview" width="100%">
  </a>
</p>

<p align="justify">
To try a screen without creating a project, open the <a href="https://nativelayoutpad.vercel.app/">online editor</a>: code, data (JSON) and a live preview in your browser, with nothing to install. There is also an editor app for Android: download <code>nativelayout-debug.apk</code> from <a href="https://github.com/getnativelayout/nativelayout/releases">Releases</a>. Every push also builds it, see the Actions tab and <a href="CONTRIBUTING.md#11-continuous-integration">CONTRIBUTING.md, section 11</a>.
</p>

## Test editor

<p align="justify">
Screenshots of the test editor app (<code>android/app</code>), with the code, the data and the live preview.
</p>

<table>
  <tr>
    <td align="center" width="33%">
      <img src="img/nl.jpg" alt="Test editor: .nl file" width="100%"><br>
      <sub><b>.nl</b></sub>
    </td>
    <td align="center" width="33%">
      <img src="img/Css.jpg" alt="Test editor: CSS" width="100%"><br>
      <sub><b>CSS</b></sub>
    </td>
    <td align="center" width="33%">
      <img src="img/output.jpg" alt="Test editor: live preview" width="100%"><br>
      <sub><b>Output</b></sub>
    </td>
  </tr>
</table>

## Install (Android)

<p align="justify">
Published through <a href="https://jitpack.io/#getnativelayout/nativelayout">JitPack</a>. First add the JitPack repository (<code>https://jitpack.io</code>) to your project:
</p>

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        maven("https://jitpack.io")
    }
}
```

Then add the dependency:

```kotlin
// app/build.gradle.kts
dependencies {
    implementation("com.github.getnativelayout.nativelayout:nativelayout-android:v1.2.0")
    implementation("com.github.getnativelayout.nativelayout:nativelayout-core:v1.2.0")
}
```

<p align="justify">
<code>nativelayout-android</code> and <code>nativelayout-core</code> are both needed, with the same version. To use only the parser and Expander (pure JVM), depend on <code>nativelayout-core</code> alone. Minimum SDK: 21.
</p>

<p align="justify">
Quick start and examples: <a href="android/README.md"><code>android/README.md</code></a> and <a href="android/examples/"><code>android/examples/</code></a>.
</p>

## API

<p align="justify">
An app is a set of screen functions. Each one holds its state, reads the taps and returns <code>view(...)</code> or <code>go { ... }</code>; the entry point only starts the first one (see <a href="SPEC.md"><code>SPEC.md</code></a>, section 8). The same verbs exist in every language. Data passed to <code>view</code> is plain: text, number, bool, list and map.
</p>

| Verb | Does |
|---|---|
| `screen(name)` | base layer: loads a `.nl` file and returns a screen |
| `state(initial)` | the state of the screen: the same object on every call, a new one after `go` |
| `view(name, data)` | returns the screen to show, with its data |
| `go(function)` | jumps to another screen function; the current one stays behind it |
| `goRoot(function)` | like `go`, and forgets the screens behind (after a login or a logout) |
| `goBack()` | the opposite of `go`: returns to the screen that jumped here |
| `canGoBack()` | `true` when there is a screen to return to |
| `back()` | `true` when the system back button woke the program; a screen that never asks gets `goBack()` |
| `render(screen, data)` | base layer: shows the screen with the data, waits for a tap, returns `true`; `false` when the window closes |
| `click(target)` | `true` if the `id` (or `".class"`) was tapped |
| `clickIndex()` | position, from 0, of the tapped element of the class |
| `clicked(name)` | the item of the `for name in ...` that holds the tapped element |
| `value(id)` | text of a field, `""` if the id does not exist |
| `value(id, item...)` | text of the field `id` of an item (one item for each enclosing `for`) |
| `checked(id)` | state of a checkbox, `false` if it does not exist |
| `checked(id, item...)` | state of the checkbox `id` of an item |
| `setValue(id, text)` | fills or clears a field (`"1"` / `"0"` for a checkbox) |
| `setValue(id, item..., text)` | fills the field `id` of an item |
| `save(key, text)` | stores text on the device |
| `load(key)` | reads stored text |
| `exists(key)` | `true` if the key is stored |
| `delete(key)` | removes a stored key |
| `close()` | releases the host |

## The file format

<p align="justify">
A screen is one <code>.nl</code> file: zero or more linked stylesheets, an optional <code>&lt;style&gt;</code>, and exactly one <code>&lt;body&gt;</code>, all wrapped in <code>&lt;nativelayout&gt;</code>.
</p>

```
<nativelayout>
    <link rel="stylesheet" href="theme.css">   (zero or more)
    <style> ... </style>                       (optional)
    <body> ... </body>                         (exactly one)
</nativelayout>
```

### Tags

| Tag | Native component |
|---|---|
| `div` | container |
| `span`, `p`, `h1`, `h2`, `h3` | text |
| `button` | button |
| `input` (`text`, `password`, `number`, `checkbox`) | field or checkbox |
| `textarea` | multi-line field |
| `img` | image (`data:` URI, a built-in icon `icon:<name>`, or a name resolved by the host) |
| `header` | bar fixed at the top of the screen, outside the scroll; direct child of the root, one per screen |
| `ul`, `li` | list with bullets |
| `include` | another `.nl` file in this place, with parameters (alone on its line, no closing tag) |
| `fieldset`, `legend` | bordered group with a title; `disabled` blocks everything inside |
| `dialog` | layer over the screen: dialog or bottom sheet |

### Style

- Selectors: `:root`, `body`, tag, `.class`, `#id`, comma lists, and the states `:pressed`, `:disabled` and `:focus`. Variables with `var(--name)`. `@media (prefers-color-scheme: dark)` for the dark theme.
- Layout: `display`, `flex-direction`, `flex`, `flex-wrap`, `justify-content` (including `space-between`, `space-around`, `space-evenly`), `align-items`, `align-self`, `gap`, `width`, `height`, `min-width`, `max-width`, `min-height`, `max-height`, `overflow`, `opacity`.
- Box: `padding` and `margin` (and each side), `border` (and `border-width`, `border-color`, `border-style`), `border-radius` (one to four values, or one corner with `border-top-left-radius` and the others), `box-shadow`.
- Background: `color`, `background` with a color or a `linear-gradient`.
- Text: `font-size`, `font-weight`, `font-style`, `font-family`, `text-align`, `text-decoration`, `text-transform`, `line-height`, `letter-spacing`.
- Animation: `animation` on `body` and `dialog` (`fade`, `slide-up`, `slide-left`, `none`, and a time). Screens and dialogs fade in by default.
- Units: `rem`, `px` (dp) and `0`. Percentages in `width`, `height`, `opacity` and `line-height`.
- Not supported: descendant and child selectors, `:hover`, attribute selectors, other `@media` conditions, `<script>`.

<p align="justify">
Values (<code>{name}</code>), control lines (<code>if</code>, <code>elif</code>, <code>else</code>, <code>for</code>, <code>end</code>), the cascade rules, error messages and complete examples are in <a href="SPEC.md"><code>SPEC.md</code></a>.
</p>

## How it works

```
panel.nl + data  ->  Expander  ->  HTML with inline styles  ->  Engine  ->  native views
                     (if, for,                                  (parse,      (host)
                      {value}, selectors)                        diff, style)
```

- **Expander:** evaluates control lines and `{values}` with your data and applies the selectors.
- **Engine:** HTML parser, style reader, neutral tree and diff.
- **Host:** creates and updates the native components.

## Repository layout

<p align="center">
  <img src="img/structure.svg" alt="Repository layout: android/ (core, android, app, examples), the specification (SPEC.md, docs/spec/) and the project files" width="100%">
</p>

## Contributing

<p align="justify">
Issues and pull requests are welcome. A new language implementation needs an Expander, an Engine and a native host that follow <code>SPEC.md</code>; the <code>.nl</code> files must work unchanged. Put it in its own folder, with its own README and examples. By contributing you agree to the licenses below (see <a href="CONTRIBUTING.md#13-license-and-ownership">CONTRIBUTING.md, section 13</a>).
</p>

## License

<p align="justify">
Copyright (c) 2026 Adilson C. Rafael.
</p>

<p align="justify">
<strong>The code</strong> is licensed under the <a href="LICENSE">Mozilla Public License 2.0</a> (MPL 2.0). In practice:
</p>

- You can use the library in any app, open or closed, free or paid.
- If you modify the files of the library itself and distribute them, you must publish those modifications under the same license.
- Your credit and copyright notices must be kept.

<p align="justify">
<strong>The specification</strong> (<a href="SPEC.md"><code>SPEC.md</code></a>) and the written documentation are licensed under <a href="LICENSE-SPEC">CC BY-SA 4.0</a>: you may copy and adapt them if you give credit and share your adaptation under the same license.
</p>

<p align="justify">
<strong>The name and the logo.</strong> The names <em>nativelayout</em> and <em>getnativelayout</em>, and the logo, are not covered by these licenses. Do not use them to name a modified version or to suggest that it is endorsed by this project.
</p>