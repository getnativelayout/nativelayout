/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-15
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

class ExpandedScreen(
    val html: String,
    val lineMap: List<Int>,
    /** Things that work but deserve a look, with the line of the `.nl` file (line numbers are the file's own). */
    val warnings: List<LayoutWarning> = emptyList()
)

class ExpandError(
    message: String,
    line: Int,
    source: String,
    word: String? = null,
    suggestion: String? = null
) : NativeLayoutError(message, line, extractLine(source, line), word, suggestion)

object Expander {
    private val cache = HashMap<String, Any>()

    fun expand(
        source: String,
        data: Map<String, Any?>,
        resolve: (String) -> String? = { null }
    ): ExpandedScreen = expand(source, data, resolve, false)

    /**
     * [dark] selects the rules inside `@media (prefers-color-scheme: dark)` (and drops the
     * ones inside `... light`). The host reads it from the system theme.
     */
    fun expand(
        source: String,
        data: Map<String, Any?>,
        resolve: (String) -> String?,
        dark: Boolean
    ): ExpandedScreen = ScreenExpander(source, data, resolve, dark).expand()

    internal fun cached(key: String): Any? = synchronized(cache) { cache[key] }

    internal fun store(key: String, prepared: Any) {
        synchronized(cache) {
            if (cache.size >= 32) cache.clear()
            cache[key] = prepared
        }
    }

    fun clearCache() = synchronized(cache) { cache.clear() }
}

private const val KIND_TAG = 1
private const val KIND_CLASS = 2
private const val KIND_ID = 3

private const val STATE_NONE = 0
private const val STATE_PRESSED = 1
private const val STATE_DISABLED = 2
private const val STATE_FOCUS = 3

private val STATE_ATTRIBUTES = arrayOf("style-pressed", "style-disabled", "style-focus")

private class Rule(
    val kind: Int,
    val name: String,
    val declarations: List<Pair<String, String>>,
    val order: Int,
    val line: Int,
    val file: String?,
    val linkLine: Int,
    val state: Int = STATE_NONE
) {
    var resolved: List<Pair<String, String>> = emptyList()
}

private class Attr(val key: String, val value: String?)

private val MISSING = Any()

private class Scope(val parent: Scope?, val vars: Map<String, Any?>) {
    fun find(name: String): Any? {
        var s: Scope? = this
        while (s != null) {
            if (s.vars.containsKey(name)) return s.vars[name]
            s = s.parent
        }
        return MISSING
    }

    fun has(name: String): Boolean = vars.containsKey(name) || (parent?.has(name) ?: false)
    fun get(name: String): Any? = if (vars.containsKey(name)) vars[name] else parent?.get(name)
}

private sealed class Node
private class TextNode(val text: String, val line: Int) : Node() {
    var parts: Array<Part>? = null
}
private class Branch(val condition: String?, val line: Int) {
    val nodes = ArrayList<Node>()
    var tokens: List<String>? = null
}
private class IfNode(val line: Int) : Node() {
    val branches = ArrayList<Branch>()
}
private class ForNode(val line: Int, val variable: String, val listName: String) : Node() {
    val nodes = ArrayList<Node>()
}
private class Frame(val openLine: Int, var nodes: MutableList<Node>, val ifNode: IfNode?)
private class Control(val type: String, val argument: String, val variable: String = "")

private val KEY_FIELDS = listOf("key", "id")
private val VOID_NAMES = setOf("br", "hr", "img", "input", "meta", "link")
private val HAS_KEY = Regex("""\skey\s*=""")
private val FIELD_OR_ID = Regex("""<(?:input|textarea)\b|\sid\s*=""")

private object Rx {
    val tagRegex = Regex("""<[^<>]*>""")
    val tagNameRegex = Regex("""^<([A-Za-z][A-Za-z0-9-]*)""")
    val attrRegex = Regex("""([A-Za-z_:][-A-Za-z0-9_:.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?""")
    val braceRegex = Regex("""\\[{}]|\{([^{}]*)\}""")
    val nameRegex = Regex("""^[A-Za-z_][A-Za-z0-9_.]*$""")
    val singleValueRegex = Regex("""^\{\s*([A-Za-z_][A-Za-z0-9_.]*)\s*\}$""")
    val varRegex = Regex("""var\(\s*(--[A-Za-z0-9_-]+)\s*\)""")
    val ruleRegex = Regex("""([^{}]+)\{([^{}]*)\}""")
    val commentRegex = Regex("""/\*[\s\S]*?\*/""")
    val forRegex = Regex("""^for\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\s+([A-Za-z_][A-Za-z0-9_.]*):$""")
    val keywordRegex = Regex("""^(if|elif|for)\s""")
    val elseSpaceRegex = Regex("""^else\s+:$""")
    val tokenRegex = Regex("""==|!=|<=|>=|<|>|\(|\)|"[^"]*"|-?\d+(?:\.\d+)?|[A-Za-z_][A-Za-z0-9_.]*""")
    val tagSelector = Regex("""[A-Za-z][A-Za-z0-9]*""")
    val classSelector = Regex("""\.[A-Za-z_][A-Za-z0-9_-]*""")
    val idSelector = Regex("""#[A-Za-z_][A-Za-z0-9_-]*""")
    val comparators = setOf("==", "!=", "<", ">", "<=", ">=")
}

private sealed class Part
private class Lit(val s: String) : Part()
private class Val(val inner: String) : Part() {
    val name = inner.trim()
    val ok = Rx.nameRegex.matches(name)
    val path: List<String> = name.split(".")
}
private class AttrPlan(
    val orig: String,
    val key: String,
    val value: String?,
    val brace: Boolean,
    val single: String?,
    val textParts: Array<Part>?
)
private class DynTag(
    val name: String,
    val lower: String,
    val selfClose: Boolean,
    val ruleStyle: String,
    val attrs: Array<AttrPlan>,
    val stateStyles: Array<String> = arrayOf("", "", "")
) : Part()

private class Prepared(val rules: List<Rule>, val nodes: List<Node>)

private val EMPTY_SCOPE = Scope(null, emptyMap())

private class ScreenExpander(
    private val source: String,
    private val data: Map<String, Any?>,
    private val resolve: (String) -> String?,
    private val dark: Boolean
) {

    private val lines = source.split("\n")
    private var rules: List<Rule> = ArrayList()
    private val variables = LinkedHashMap<String, String>()
    private var ruleOrder = 0
    private val out = ArrayList<String>()
    private val outLines = ArrayList<Int>()
    private val warnings = ArrayList<LayoutWarning>()


    private fun err(message: String, line: Int, word: String? = null, suggestion: String? = null) =
        ExpandError(message, line, source, word, suggestion)

    private fun styleError(message: String, line: Int, word: String?, file: String?, linkLine: Int): ExpandError =
        if (file == null) err("style: $message", line, word)
        else err("style: in '$file' line $line: $message", linkLine, word)

    private fun linkHref(tag: String, line: Int): String {
        val attrs = HashMap<String, String>()
        val inner = tag.removePrefix("<link").removeSuffix(">").removeSuffix("/")
        for (a in Rx.attrRegex.findAll(inner)) {
            val value = a.groups[2]?.value ?: a.groups[3]?.value ?: a.groups[4]?.value ?: continue
            attrs[a.groupValues[1].lowercase()] = value
        }
        val href = attrs["href"]
        if (href == null || href.isBlank() || attrs["rel"]?.lowercase() != "stylesheet") {
            throw err("markup: <link> needs rel=\"stylesheet\" and a href", line)
        }
        if (href.startsWith("/") || href.contains("..") || href.contains("://")) {
            throw err("style: the file '$href' is not allowed", line, href)
        }
        if (!href.endsWith(".css")) throw err("style: the file '$href' must be a .css file", line, href)
        return href
    }

    fun expand(): ExpandedScreen {
        var first = 0
        while (first < lines.size && lines[first].isBlank()) first++
        var last = lines.size - 1
        while (last >= 0 && lines[last].isBlank()) last--
        if (first >= last || lines[first].trim() != "<nativelayout>" || lines[last].trim() != "</nativelayout>") {
            throw err("the file must start with <nativelayout> and end with </nativelayout>", if (first < lines.size) first + 1 else 1)
        }

        var styleStart = -1
        var styleEnd = -1
        var bodyStart = -1
        var bodyEnd = -1
        val links = ArrayList<Pair<String, Int>>()
        var i = first + 1
        var inComment = false
        while (i < last) {
            val t = lines[i].trim()
            when {
                inComment -> {
                    if (t.contains("-->")) inComment = false
                }
                t.isEmpty() -> {
                }
                t.startsWith("<!--") -> {
                    if (!t.contains("-->")) inComment = true
                }
                t.startsWith("<script") -> throw err("markup: <script> is not allowed", i + 1)
                t == "<style>" -> {
                    if (styleStart >= 0) throw err("only one <style> block is allowed", i + 1)
                    var j = i + 1
                    while (j < last && lines[j].trim() != "</style>") j++
                    if (j >= last) throw err("<style> is never closed with </style>", i + 1)
                    styleStart = i + 1
                    styleEnd = j
                    i = j
                }
                t.startsWith("<link") -> {
                    if (bodyStart >= 0) throw err("a <link> must come before <body>", i + 1)
                    links.add(Pair(linkHref(t, i + 1), i + 1))
                }
                t.startsWith("<body") -> {
                    if (bodyStart >= 0) throw err("the file must have exactly one <body>", i + 1)
                    var j = i + 1
                    while (j < last && lines[j].trim() != "</body>") j++
                    if (j >= last) throw err("<body> is never closed with </body>", i + 1)
                    bodyStart = i
                    bodyEnd = j
                    i = j
                }
                else -> throw err("only <style> and <body> are allowed inside <nativelayout>", i + 1)
            }
            i++
        }
        if (bodyStart < 0) throw err("the file must have exactly one <body>", first + 1)

        val loaded = HashSet<String>()
        val sheets = ArrayList<Triple<String, Int, String>>()
        for ((href, linkLine) in links) {
            if (!loaded.add(href)) continue
            val css = resolve(href) ?: throw err("style: the file '$href' was not found", linkLine, href)
            if (css.contains("@import")) throw err("style: a style file cannot import another file", linkLine, href)
            sheets.add(Triple(href, linkLine, css))
        }

        val key = StringBuilder(source.length + 64).append(if (dark) 'd' else 'l').append('\u0001').append(source)
        for (sheet in sheets) key.append('\u0000').append(sheet.third)
        val cacheKey = key.toString()

        val hit = Expander.cached(cacheKey) as? Prepared
        val prepared: Prepared
        if (hit != null) {
            prepared = hit
            rules = hit.rules
        } else {
            for (sheet in sheets) parseStyle(sheet.third, 1, sheet.first, sheet.second)
            if (styleStart >= 0) parseStyle(lines.subList(styleStart, styleEnd).joinToString("\n"), styleStart + 1)
            resolveVariables()
            prepared = Prepared(rules, buildNodes(bodyStart, bodyEnd))
            Expander.store(cacheKey, prepared)
        }
        emit(prepared.nodes, Scope(null, data))
        return ExpandedScreen(out.joinToString("\n"), outLines, warnings.toList())
    }

    private fun flattenMedia(text: String, firstLine: Int, file: String?, linkLine: Int): String {
        var t = text
        while (true) {
            val at = t.indexOf("@media")
            if (at < 0) return t
            val line = firstLine + t.substring(0, at).count { it == '\n' }
            val open = t.indexOf('{', at)
            if (open < 0) throw styleError("'@media' without '{'", line, "@media", file, linkLine)
            var depth = 0
            var close = -1
            var i = open
            while (i < t.length) {
                val c = t[i]
                if (c == '{') depth++
                else if (c == '}') {
                    depth--
                    if (depth == 0) { close = i; break }
                }
                i++
            }
            if (close < 0) throw styleError("the '@media' block is never closed", line, "@media", file, linkLine)
            val condition = t.substring(at + 6, open).trim().lowercase().replace(Regex("\\s+"), " ")
            val include = when (condition.replace(": ", ":")) {
                "(prefers-color-scheme:dark)" -> dark
                "(prefers-color-scheme:light)" -> !dark
                else -> throw styleError(
                    "the '@media $condition' is not supported " +
                        "(use '@media (prefers-color-scheme: dark)' or '... light')", line, "@media", file, linkLine
                )
            }
            val replacement = if (include) t.substring(open + 1, close) else t.substring(at, close + 1).filter { it == '\n' }
            t = t.substring(0, at) + replacement + t.substring(close + 1)
        }
    }

    private fun parseStyle(text: String, firstLine: Int, file: String? = null, linkLine: Int = 0) {
        val commentless = Rx.commentRegex.replace(text) { m -> m.value.filter { it == '\n' } }
        val clean = flattenMedia(commentless, firstLine, file, linkLine)
        for (m in Rx.ruleRegex.findAll(clean)) {
            val selectorText = m.groupValues[1]
            val offset = m.range.first + (selectorText.length - selectorText.trimStart().length)
            val line = firstLine + clean.substring(0, offset).count { it == '\n' }
            val declarations = ArrayList<Pair<String, String>>()
            for (part in m.groupValues[2].split(";")) {
                val idx = part.indexOf(':')
                if (idx < 0) continue
                val name = part.substring(0, idx).trim().lowercase()
                val value = part.substring(idx + 1).replace("!important", "").trim()
                if (name.isEmpty() || value.isEmpty()) continue
                declarations.add(Pair(name, value))
            }
            for (raw in selectorText.split(",")) {
                val full = raw.trim()
                var selector = full
                var state = STATE_NONE
                val colon = full.indexOf(':')
                if (colon > 0) {
                    state = when (full.substring(colon + 1).lowercase()) {
                        "pressed", "active" -> STATE_PRESSED
                        "disabled" -> STATE_DISABLED
                        "focus" -> STATE_FOCUS
                        else -> throw styleError(
                            "the selector '$full' is not supported (states: :pressed, :disabled, :focus)",
                            line, full, file, linkLine
                        )
                    }
                    selector = full.substring(0, colon)
                }
                val list = rules as ArrayList<Rule>
                when {
                    selector == ":root" && state == STATE_NONE -> {
                        for (d in declarations) if (d.first.startsWith("--")) variables[d.first] = d.second
                    }
                    Rx.tagSelector.matches(selector) -> list.add(Rule(KIND_TAG, selector.lowercase(), declarations, ruleOrder++, line, file, linkLine, state))
                    Rx.classSelector.matches(selector) -> list.add(Rule(KIND_CLASS, selector.substring(1), declarations, ruleOrder++, line, file, linkLine, state))
                    Rx.idSelector.matches(selector) -> list.add(Rule(KIND_ID, selector.substring(1), declarations, ruleOrder++, line, file, linkLine, state))
                    else -> throw styleError("the selector '$full' is not supported", line, full, file, linkLine)
                }
            }
        }
    }

    private fun substituteVars(value: String, rule: Rule): String =
        Rx.varRegex.replace(value) { m ->
            variables[m.groupValues[1]]
                ?: throw styleError("the variable '${m.groupValues[1]}' is not defined in ':root'", rule.line, m.groupValues[1], rule.file, rule.linkLine)
        }

    private fun resolveVariables() {
        for (rule in rules) {
            val resolved = ArrayList<Pair<String, String>>()
            for (d in rule.declarations) {
                if (d.first.startsWith("--")) continue
                resolved.add(Pair(d.first, substituteVars(d.second, rule)))
            }
            rule.resolved = resolved
        }
    }

    private fun parseControl(t: String, line: Int): Control? {
        if (t == "end") return Control("end", "")
        if (t == "else:") return Control("else", "")
        if (Rx.elseSpaceRegex.matches(t)) throw err("write 'else:' without a space before ':'", line)
        val keyword = Rx.keywordRegex.find(t)?.groupValues?.get(1) ?: return null
        if (!t.endsWith(":")) return null
        val body = t.substring(0, t.length - 1)
        if (body.endsWith(" ") || body.endsWith("\t")) {
            throw err("write '${body.trimEnd()}:' without a space before ':'", line)
        }
        if (keyword == "for") {
            val m = Rx.forRegex.find(t) ?: throw err("invalid 'for': '$t'", line, null, "write 'for item in list:'")
            return Control("for", m.groupValues[2], m.groupValues[1])
        }
        val condition = body.substring(keyword.length).trim()
        if (condition.isEmpty()) throw err("invalid condition: ''", line)
        return Control(keyword, condition)
    }

    private fun buildNodes(from: Int, to: Int): List<Node> {
        val root = ArrayList<Node>()
        val stack = ArrayList<Frame>()
        stack.add(Frame(0, root, null))
        for (idx in from..to) {
            val line = idx + 1
            val control = if (idx == from || idx == to) null else parseControl(lines[idx].trim(), line)
            val top = stack[stack.size - 1]
            if (control == null) {
                top.nodes.add(TextNode(lines[idx], line))
                continue
            }
            when (control.type) {
                "if" -> {
                    val node = IfNode(line)
                    val branch = Branch(control.argument, line)
                    node.branches.add(branch)
                    top.nodes.add(node)
                    stack.add(Frame(line, branch.nodes, node))
                }
                "elif", "else" -> {
                    val ifNode = top.ifNode ?: throw err("'${control.type}' has no matching 'if'", line)
                    if (ifNode.branches[ifNode.branches.size - 1].condition == null) {
                        throw err("'${control.type}' has no matching 'if'", line)
                    }
                    val branch = Branch(if (control.type == "else") null else control.argument, line)
                    ifNode.branches.add(branch)
                    top.nodes = branch.nodes
                }
                "for" -> {
                    val node = ForNode(line, control.variable, control.argument)
                    top.nodes.add(node)
                    stack.add(Frame(line, node.nodes, null))
                }
                else -> {
                    if (stack.size == 1) throw err("'end' has no matching 'if' or 'for'", line)
                    stack.removeAt(stack.size - 1)
                }
            }
        }
        if (stack.size > 1) {
            val open = stack[stack.size - 1]
            val word = if (open.ifNode != null) "if" else "for"
            throw err("the '$word' opened on line ${open.openLine} is never closed with 'end'", open.openLine)
        }
        return root
    }

    private fun emit(nodes: List<Node>, scope: Scope) {
        for (node in nodes) {
            when (node) {
                is TextNode -> {
                    val parts = node.parts ?: compileLine(node.text).also { node.parts = it }
                    out.add(renderParts(parts, node.line, scope))
                    outLines.add(node.line)
                }
                is IfNode -> {
                    for (branch in node.branches) {
                        val condition = branch.condition
                        if (condition == null || evaluate(branch, condition, scope)) {
                            emit(branch.nodes, scope)
                            break
                        }
                    }
                }
                is ForNode -> {
                    val list = lookup(node.listName, node.line, scope)
                    if (list !is List<*>) throw err("'${node.listName}' is not a list", node.line, node.listName)
                    if (scope.has(node.variable)) throw err("the name '${node.variable}' already exists", node.line, node.variable)
                    var positionWarned = false
                    for (item in list) {
                        val first = out.size
                        emit(node.nodes, Scope(scope, mapOf(node.variable to item)))
                        val keyed = applyKey(first, keyOfItem(item))
                        if (!keyed && !positionWarned && holdsFieldsOrIds(first)) {
                            positionWarned = true
                            val w = LayoutWarning("the items of this list have no id: using the position", node.line)
                            if (warnings.none { it.message == w.message && it.line == w.line }) warnings.add(w)
                        }
                    }
                }
            }
        }
    }

    /**
     * The automatic key of an item (section 5.7): its field `key`, or else its field `id`.
     * Only text and whole numbers count. Anything else gives null and the position is used.
     */
    private fun keyOfItem(item: Any?): String? {
        if (item !is Map<*, *>) return null
        for (field in KEY_FIELDS) {
            when (val v = item[field]) {
                is String -> v.trim().takeIf { it.isNotEmpty() }?.let { return it }
                is Int, is Long, is Short, is Byte -> return v.toString()
                else -> {}
            }
        }
        return null
    }

    /**
     * Writes `key="..."` on the elements at the first level of what one item of a `for` emitted,
     * unless the author already wrote a key. Returns false when there was no key to give.
     */
    private fun applyKey(first: Int, key: String?): Boolean {
        if (key == null) return false
        val escaped = escapeHtml(key)
        var depth = 0
        for (k in first until out.size) {
            val line = out[k]
            var rebuilt: StringBuilder? = null
            var copied = 0
            for (m in Rx.tagRegex.findAll(line)) {
                val tag = m.value
                if (tag.startsWith("<!")) continue
                if (tag.startsWith("</")) {
                    depth--
                    continue
                }
                val name = Rx.tagNameRegex.find(tag)?.groupValues?.get(1)?.lowercase() ?: continue
                if (depth == 0 && !HAS_KEY.containsMatchIn(tag)) {
                    val b = rebuilt ?: StringBuilder().also { rebuilt = it }
                    b.append(line, copied, m.range.first)
                    b.append(tag, 0, name.length + 1).append(" key=\"").append(escaped).append('"').append(tag, name.length + 1, tag.length)
                    copied = m.range.last + 1
                }
                if (name !in VOID_NAMES && !tag.endsWith("/>")) depth++
            }
            rebuilt?.let {
                it.append(line, copied, line.length)
                out[k] = it.toString()
            }
        }
        return true
    }

    /** True when what an item emitted has an `id` or a field, that is, something that follows the item. */
    private fun holdsFieldsOrIds(first: Int): Boolean {
        for (k in first until out.size) if (FIELD_OR_ID.containsMatchIn(out[k])) return true
        return false
    }

    private fun lookup(name: String, line: Int, scope: Scope): Any? =
        lookupPath(name, name.split("."), line, scope)

    private fun lookupPath(name: String, parts: List<String>, line: Int, scope: Scope): Any? {
        var value: Any? = scope.find(parts[0])
        if (value === MISSING) throw err("the name '$name' was not given by the program", line, parts[0])
        for (k in 1 until parts.size) {
            val part = parts[k]
            val current = value
            value = when {
                current is Map<*, *> && current.containsKey(part) -> current[part]
                part == "size" && current is Map<*, *> -> current.size
                part == "size" && current is List<*> -> current.size
                part == "size" && current is String -> current.length
                else -> throw err("the name '$name' was not given by the program", line, parts[0])
            }
        }
        return value
    }

    private fun tokenize(text: String, line: Int): List<String> {
        val tokens = ArrayList<String>()
        var pos = 0
        while (pos < text.length) {
            if (text[pos].isWhitespace()) {
                pos++
                continue
            }
            val m = Rx.tokenRegex.find(text, pos)
            if (m == null || m.range.first != pos) throw err("invalid condition: '$text'", line)
            tokens.add(m.value)
            pos = m.range.last + 1
        }
        return tokens
    }

    private fun evaluate(branch: Branch, condition: String, scope: Scope): Boolean {
        val tokens = branch.tokens ?: tokenize(condition, branch.line).also { branch.tokens = it }
        return ConditionParser(tokens, condition, branch.line, scope).run()
    }

    private inner class ConditionParser(
        val tokens: List<String>,
        val text: String,
        val line: Int,
        val scope: Scope
    ) {
        var pos = 0

        fun run(): Boolean {
            val value = parseOr()
            if (pos != tokens.size) throw bad()
            return value
        }

        fun bad(): ExpandError = err("invalid condition: '$text'", line)

        fun peek(): String? = if (pos < tokens.size) tokens[pos] else null

        fun parseOr(): Boolean {
            var v = parseAnd()
            while (peek() == "or") {
                pos++
                val r = parseAnd()
                v = v || r
            }
            return v
        }

        fun parseAnd(): Boolean {
            var v = parseNot()
            while (peek() == "and") {
                pos++
                val r = parseNot()
                v = v && r
            }
            return v
        }

        fun parseNot(): Boolean {
            if (peek() == "not") {
                pos++
                return !parseNot()
            }
            if (peek() == "(") {
                pos++
                val v = parseOr()
                if (peek() != ")") throw bad()
                pos++
                return v
            }
            return parseCompare()
        }

        fun parseCompare(): Boolean {
            val left = operand()
            val op = peek()
            if (op != null && op in Rx.comparators) {
                pos++
                val right = operand()
                return compare(op, left, right)
            }
            if (left is Boolean) return left
            throw err("invalid condition: '$text'", line, null, "a name alone must be true or false")
        }

        fun operand(): Any? {
            val token = peek() ?: throw bad()
            pos++
            if (token == "true") return true
            if (token == "false") return false
            if (token.startsWith("\"")) return token.substring(1, token.length - 1)
            if (token[0].isDigit() || token[0] == '-') return token.toDouble()
            if (token == "and" || token == "or" || token == "not" || token == "(" || token == ")" || token in Rx.comparators) throw bad()
            return lookup(token, line, scope)
        }

        fun compare(op: String, a: Any?, b: Any?): Boolean {
            val x = (a as? Number)?.toDouble()
            val y = (b as? Number)?.toDouble()
            if (x != null && y != null) {
                return when (op) {
                    "==" -> x == y
                    "!=" -> x != y
                    "<" -> x < y
                    ">" -> x > y
                    "<=" -> x <= y
                    else -> x >= y
                }
            }
            if (op == "==") return a == b
            if (op == "!=") return a != b
            if (a is String && b is String) {
                val c = a.compareTo(b)
                return when (op) {
                    "<" -> c < 0
                    ">" -> c > 0
                    "<=" -> c <= 0
                    else -> c >= 0
                }
            }
            throw err("invalid condition: '$text'", line, null, "these values cannot be compared with '$op'")
        }
    }

    private fun escapeHtml(s: String): String =
        s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

    private fun formatNumber(n: Number): String {
        if (n is Double || n is Float) {
            val d = n.toDouble()
            if (!d.isInfinite() && !d.isNaN() && d == Math.floor(d)) return d.toLong().toString()
            return d.toString()
        }
        return n.toString()
    }

    private fun singular(name: String): String {
        val last = name.substringAfterLast('.')
        return if (last.length > 1 && last.endsWith("s")) last.dropLast(1) else "item"
    }

    private fun valText(v: Val, line: Int, scope: Scope): String {
        if (!v.ok) {
            throw err("arithmetic is not supported in '{${v.inner}}'; give the program's result as a name", line)
        }
        val name = v.name
        return when (val x = lookupPath(name, v.path, line, scope)) {
            null -> ""
            is String -> escapeHtml(x)
            is Boolean -> x.toString()
            is Number -> formatNumber(x)
            is List<*> -> throw err("'{$name}' is a list; use 'for ${singular(name)} in $name:'", line, name)
            is Map<*, *> -> throw err("'{$name}' is a map; use a name inside it, like '{$name.field}'", line, name)
            else -> escapeHtml(x.toString())
        }
    }

    private fun mergeLits(list: List<Part>): Array<Part> {
        val out = ArrayList<Part>(list.size)
        for (p in list) {
            val last = out.lastOrNull()
            if (p is Lit && last is Lit) out[out.size - 1] = Lit(last.s + p.s) else out.add(p)
        }
        return out.toTypedArray()
    }

    private fun compileText(text: String, list: MutableList<Part>) {
        var pos = 0
        for (m in Rx.braceRegex.findAll(text)) {
            if (m.range.first > pos) list.add(Lit(text.substring(pos, m.range.first)))
            when (m.value) {
                "\\{" -> list.add(Lit("{"))
                "\\}" -> list.add(Lit("}"))
                else -> list.add(Val(m.groupValues[1]))
            }
            pos = m.range.last + 1
        }
        if (pos < text.length) list.add(Lit(text.substring(pos)))
    }

    private fun compileValue(text: String): Array<Part> {
        val list = ArrayList<Part>()
        compileText(text, list)
        return mergeLits(list)
    }

    private fun compileLine(text: String): Array<Part> {
        val list = ArrayList<Part>()
        var pos = 0
        for (m in Rx.tagRegex.findAll(text)) {
            compileText(text.substring(pos, m.range.first), list)
            list.add(compileTag(m.value))
            pos = m.range.last + 1
        }
        compileText(text.substring(pos), list)
        return mergeLits(list)
    }

    private fun renderParts(parts: Array<Part>, line: Int, scope: Scope): String {
        if (parts.size == 1) {
            val only = parts[0]
            if (only is Lit) return only.s
        }
        val sb = StringBuilder()
        for (p in parts) {
            when (p) {
                is Lit -> sb.append(p.s)
                is Val -> sb.append(valText(p, line, scope))
                is DynTag -> sb.append(renderDyn(p, line, scope))
            }
        }
        return sb.toString()
    }

    private fun styleFor(tag: String, classes: List<String>, id: String?, state: Int = STATE_NONE): String {
        val matched = rules.filter {
            it.state == state && when (it.kind) {
                KIND_TAG -> it.name == tag
                KIND_CLASS -> it.name in classes
                else -> id != null && it.name == id
            }
        }.sortedWith(compareBy<Rule>({ it.kind }, { it.order }))
        val parts = ArrayList<String>()
        for (rule in matched) for (d in rule.resolved) parts.add(d.first + ": " + d.second)
        return parts.joinToString("; ")
    }

    private fun compileTag(tag: String): Part {
        if (tag.startsWith("<!")) return Lit(tag)
        if (tag.startsWith("</")) {
            val closing = tag.substring(2, tag.length - 1).trim().lowercase()
            return Lit(if (closing == "body") "</div>" else tag)
        }
        val nameMatch = Rx.tagNameRegex.find(tag) ?: return Lit(tag)
        val name = nameMatch.groupValues[1]
        val lower = name.lowercase()
        val selfClose = tag.endsWith("/>")
        val inner = tag.substring(nameMatch.range.last + 1, tag.length - (if (selfClose) 2 else 1))

        val attrs = ArrayList<AttrPlan>()
        var classes: List<String> = emptyList()
        var id: String? = null
        var anyBrace = false
        for (a in Rx.attrRegex.findAll(inner)) {
            val key = a.groupValues[1].lowercase()
            val value: String? = a.groups[2]?.value ?: a.groups[3]?.value ?: a.groups[4]?.value
            val brace = value != null && value.contains("{")
            var single: String? = null
            var textParts: Array<Part>? = null
            if (brace) {
                anyBrace = true
                single = Rx.singleValueRegex.find(value!!.trim())?.groupValues?.get(1)
                textParts = compileValue(value)
            } else {
                if (key == "class") classes = (value ?: "").split(" ").filter { it.isNotEmpty() }
                if (key == "id") id = value
            }
            attrs.add(AttrPlan(a.groupValues[1], key, value, brace, single, textParts))
        }
        val plan = DynTag(
            name, lower, selfClose, styleFor(lower, classes, id), attrs.toTypedArray(),
            arrayOf(
                styleFor(lower, classes, id, STATE_PRESSED),
                styleFor(lower, classes, id, STATE_DISABLED),
                styleFor(lower, classes, id, STATE_FOCUS)
            )
        )
        return if (anyBrace) plan else Lit(renderDyn(plan, 0, EMPTY_SCOPE))
    }

    private fun renderDyn(t: DynTag, line: Int, scope: Scope): String {
        val result = ArrayList<Attr>(t.attrs.size + 1)
        var inline = ""
        for (a in t.attrs) {
            var value: String? = a.value
            if (a.brace) {
                if (a.key == "id" || a.key == "class") throw err("markup: '${a.key}' cannot be generated by value", line, a.key)
                val single = a.single
                if ((a.key == "checked" || a.key == "disabled" || a.key == "readonly") && single != null) {
                    val flag = lookup(single, line, scope)
                    if (flag == true) result.add(Attr(a.orig, "true"))
                    continue
                }
                value = renderParts(a.textParts!!, line, scope)
            }
            if (a.key == "style") inline = value ?: "" else result.add(Attr(a.orig, value))
        }

        val parts = ArrayList<String>()
        if (t.ruleStyle.isNotEmpty()) parts.add(t.ruleStyle)
        val inlineStyle = inline.trim().trimEnd(';').trim()
        if (inlineStyle.isNotEmpty()) parts.add(inlineStyle)
        if (parts.isNotEmpty()) result.add(Attr("style", parts.joinToString("; ")))
        for (k in 0 until 3) if (t.stateStyles[k].isNotEmpty()) result.add(Attr(STATE_ATTRIBUTES[k], t.stateStyles[k]))

        val sb = StringBuilder("<")
        sb.append(if (t.lower == "body") "div" else t.name)
        for (a in result) {
            sb.append(' ').append(a.key)
            val v = a.value
            if (v != null) sb.append("=\"").append(v.replace("\"", "&quot;")).append('"')
        }
        sb.append(if (t.selfClose) " />" else ">")
        return sb.toString()
    }
}
