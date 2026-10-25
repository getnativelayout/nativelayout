/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-15
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color
import android.graphics.Outline
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Rect
import android.graphics.RectF
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.graphics.drawable.RippleDrawable
import android.graphics.drawable.StateListDrawable
import android.os.Build
import android.text.InputType
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.AbsoluteSizeSpan
import android.text.style.ForegroundColorSpan
import android.text.style.StrikethroughSpan
import android.text.style.StyleSpan
import android.text.style.UnderlineSpan
import android.util.AttributeSet
import android.util.Base64
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.view.ViewTreeObserver
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import kotlin.coroutines.Continuation
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine
import kotlin.math.roundToInt
import nativelayout.core.*

class AndroidLayoutHost(
    private val context: Context,
    private val areaProvider: () -> FrameLayout,
    private val imageResolver: (String) -> Bitmap? = { null }
) : LayoutHost {
    private class ViewEntry(
        val view: View,
        val container: ViewGroup?,
        var style: NativeStyle,
        var node: Native,
        var list: VirtualList? = null
    )

    private val entries = HashMap<String, ViewEntry>()
    private var rootKey: String? = null
    private var area: FrameLayout? = null

    /** Views of the <dialog> layers: they sit over the screen, in the area, and not inside the scroll. */
    private val pendingLayers = ArrayList<View>()
    private var layersReady = false

    /** The <header>: it sits above the scroll, in its own holder, and does not scroll with the screen. */
    private var pendingHeader: View? = null
    private var headerHolder: FrameLayout? = null
    private var column: LinearLayout? = null

    /**
     * The <footer>: with dock bottom (the default) it sits under the scroll and takes its space;
     * with the other docks it floats over the screen, in the area.
     */
    private var footerHolder: FrameLayout? = null
    private val pendingFooters = ArrayList<View>()

    private val queue = ArrayDeque<LayoutEvent>()
    private var pending: Continuation<LayoutEvent?>? = null
    private var closed = false

    private val prefs by lazy { context.getSharedPreferences("nativelayout", Context.MODE_PRIVATE) }

    private val density: Float get() = context.resources.displayMetrics.density
    private val fontScale: Float get() = context.resources.configuration.fontScale

    private fun px(c: Length): Int = c.toPixels(density, fontScale).roundToInt()
    private fun pxF(c: Length): Float = c.toPixels(density, fontScale)

    override fun applyChanges(changes: List<Change>) {
        if (closed) return
        for (a in changes) {
            when (a) {
                is Change.SetRoot -> setRoot(a.node)
                is Change.Create -> create(a)
                is Change.Remove -> remove(a.key)
                is Change.Replace -> replace(a)
                is Change.UpdateText -> updateText(a)
                is Change.UpdateStyle -> updateStyle(a)
                is Change.UpdateAttributes -> updateAttributes(a)
                is Change.SetItems -> entries[a.key]?.list?.setModel(a.items)
            }
        }
    }

    override suspend fun nextEvent(): LayoutEvent? {
        if (queue.isNotEmpty()) return queue.removeFirst()
        if (closed) return null
        return suspendCoroutine<LayoutEvent?> { continuation -> pending = continuation }
    }

    override fun fieldText(id: String): String? =
        (entries["#$id"]?.view as? EditText)?.text?.toString()

    override fun fieldChecked(id: String): Boolean? =
        (entries["#$id"]?.view as? CheckBox)?.isChecked

    override fun setValue(id: String, text: String) {
        when (val v = entries["#$id"]?.view) {
            is EditText -> {
                v.setText(text)
                v.setSelection(v.text.length)
            }
            is CheckBox -> v.isChecked = text.trim().lowercase() in setOf("1", "true", "on")
            else -> {}
        }
    }

    override fun save(key: String, text: String) {
        prefs.edit().putString(key, text).apply()
    }

    override fun load(key: String): String? = prefs.getString(key, null)

    override fun exists(key: String): Boolean = prefs.contains(key)

    override fun delete(key: String) {
        prefs.edit().remove(key).apply()
    }

    override fun close() {
        if (closed) return
        closed = true
        val continuation = pending
        pending = null
        queue.clear()
        reset()
        area = null
        continuation?.resume(null)
    }

    private fun publish(key: String) {
        if (closed) return
        val continuation = pending
        if (continuation != null) {
            pending = null
            continuation.resume(LayoutEvent(key))
        } else {
            queue.addLast(LayoutEvent(key))
        }
    }

    /** The system back button was pressed. Call it on the UI thread. */
    fun postBack() {
        publish(BACK_EVENT_KEY)
    }

    private fun reset() {
        entries.clear()
        rootKey = null
        pendingLayers.clear()
        layersReady = false
        pendingHeader = null
        headerHolder = null
        column = null
        footerHolder = null
        pendingFooters.clear()
        area?.removeAllViews()
    }

    private fun setRoot(node: Native) {
        reset()
        val target = areaProvider()
        area = target
        val entry = createView(node, null)
        // before Android 12 there is no stretch effect: ElasticScrollView draws the rubber band itself
        val scroll: ScrollView =
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) ElasticScrollView(context) else ScrollView(context)
        scroll.apply {
            isFillViewport = true
            addView(entry.view, rootParams(node.style))
        }
        val holder = FrameLayout(context)
        val col = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(node.style.backgroundColor)
            addView(holder, LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT
            ))
            addView(scroll, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        }
        val bottom = FrameLayout(context)
        col.addView(bottom, LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT
        ))
        footerHolder = bottom
        headerHolder = holder
        column = col
        pendingHeader?.let { holder.addView(it, headerParams(it)) }
        pendingHeader = null
        target.removeAllViews()
        target.addView(col, FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT
        ))
        layersReady = true
        for (footer in pendingFooters) attachFooter(footer)
        pendingFooters.clear()
        for (layer in pendingLayers) target.addView(layer, layerParams())
        pendingLayers.clear()
        rootKey = node.key
        animateIn(col, node.style.animation ?: DefaultAnimations.SCREEN, false)
    }

    private fun isLayer(n: Native): Boolean = n is NativeBox && n.overlay

    private fun isHeader(n: Native): Boolean = n is NativeBox && n.header

    private fun headerParams(view: View): FrameLayout.LayoutParams {
        val style = (view.tag as? String)?.let { entries[it]?.style }
        return if (style != null) rootParams(style)
        else FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
    }

    /** The <header> goes above the scroll. While the screen is being built it waits. */
    private fun attachHeader(view: View) {
        val holder = headerHolder
        if (layersReady && holder != null) holder.addView(view, headerParams(view)) else pendingHeader = view
    }

    private fun isFooter(n: Native): Boolean = n is NativeBox && n.footer

    private fun alignGravity(a: Align?, vertical: Boolean): Int = when (a) {
        Align.START -> if (vertical) Gravity.TOP else Gravity.START
        Align.CENTER -> if (vertical) Gravity.CENTER_VERTICAL else Gravity.CENTER_HORIZONTAL
        Align.END -> if (vertical) Gravity.BOTTOM else Gravity.END
        else -> Gravity.NO_GRAVITY
    }

    /** Size, margin and place of a <footer>, from its style and its `dock`. */
    private fun footerParams(e: NativeStyle): FrameLayout.LayoutParams {
        val dock = e.dock ?: Dock.BOTTOM
        val wide = dock == Dock.BOTTOM || dock == Dock.TOP
        val w = e.width
        val h = e.height
        val width = when {
            w is Size.Dim -> px(w.length)
            w is Size.Percent && w.value >= 100f -> ViewGroup.LayoutParams.MATCH_PARENT
            wide -> ViewGroup.LayoutParams.MATCH_PARENT
            else -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        val height = when {
            h is Size.Dim -> px(h.length)
            h is Size.Percent && h.value >= 100f -> ViewGroup.LayoutParams.MATCH_PARENT
            else -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        val gravity = when (dock) {
            Dock.BOTTOM -> Gravity.BOTTOM or alignGravity(e.alignSelf, false)
            Dock.TOP -> Gravity.TOP or alignGravity(e.alignSelf, false)
            Dock.LEFT -> Gravity.START or alignGravity(e.alignSelf, true).let { if (it == Gravity.NO_GRAVITY) Gravity.CENTER_VERTICAL else it }
            Dock.RIGHT -> Gravity.END or alignGravity(e.alignSelf, true).let { if (it == Gravity.NO_GRAVITY) Gravity.CENTER_VERTICAL else it }
            Dock.CENTER -> Gravity.CENTER
        }
        return FrameLayout.LayoutParams(width, height, gravity).apply {
            setMargins(px(e.margin.left), px(e.margin.top), px(e.margin.right), px(e.margin.bottom))
        }
    }

    /** The <footer> goes under the scroll (dock bottom) or over the screen (any other dock). */
    private fun attachFooter(view: View) {
        val style = (view.tag as? String)?.let { entries[it]?.style } ?: NativeStyle()
        val dock = style.dock ?: Dock.BOTTOM
        val params = footerParams(style)
        val bottom = footerHolder
        val target = area
        if (!layersReady || bottom == null || target == null) {
            pendingFooters.add(view)
            return
        }
        if (dock == Dock.BOTTOM) bottom.addView(view, params) else target.addView(view, params)
    }

    private fun layerParams() = FrameLayout.LayoutParams(
        ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT
    )

    /** A <dialog> goes over everything in the area. While the screen is being built it waits in a list. */
    private fun attachLayer(view: View) {
        val target = area
        if (layersReady && target != null) {
            target.addView(view, layerParams())
            val style = (view.tag as? String)?.let { entries[it]?.style }
            animateIn(view, style?.animation ?: DefaultAnimations.LAYER, true)
        } else {
            pendingLayers.add(view)
        }
    }

    /**
     * Entrance animation of a screen (the whole column) or of a <dialog> layer. The motion itself
     * (a fade and a damped spring) is in [ScreenMotion]; here only `none` is decided.
     */
    private fun animateIn(view: View, anim: Animation, layer: Boolean) {
        if (anim.kind == AnimKind.NONE || anim.durationMs <= 0) return
        ScreenMotion.enter(view, anim, layer, density)
    }

    private fun create(a: Change.Create) {
        val parentEntry = entries[a.parentKey] ?: return
        val container = parentEntry.container ?: return
        val created = createView(a.node, parentEntry.style)
        if (isLayer(a.node)) {
            attachLayer(created.view)
            return
        }
        if (isHeader(a.node)) {
            attachHeader(created.view)
            return
        }
        if (isFooter(a.node)) {
            attachFooter(created.view)
            return
        }
        if (a.node.style.position == Position.ABSOLUTE && parentEntry.view is AbsFrame) {
            parentEntry.view.addAbs(created.view, a.position)
            return
        }
        // the <header> is the first child of the root in the tree, but it is not a view of the root
        val shift = if (a.parentKey == rootKey && (headerHolder?.childCount ?: 0) > 0) 1 else 0
        container.addView(created.view, (a.position - shift).coerceIn(0, container.childCount))
        applyPercents(created.view, created.style, container)
    }

    private fun remove(key: String) {
        val entry = entries[key] ?: return
        if (key == rootKey) {
            reset()
            return
        }
        discard(entry.view)
        (entry.view.parent as? ViewGroup)?.removeView(entry.view)
    }

    private fun replace(a: Change.Replace) {
        val old = entries[a.key] ?: return
        if (a.key == rootKey) {
            setRoot(a.node)
            return
        }
        val parent = old.view.parent as? ViewGroup ?: return
        val index = parent.indexOfChild(old.view)
        val parentEntry = entryOfParent(parent)
        val oldWasLayer = isLayer(old.node)
        val oldWasHeader = isHeader(old.node)
        val oldWasFooter = isFooter(old.node)
        discard(old.view)
        parent.removeView(old.view)
        when {
            isLayer(a.node) -> attachLayer(createView(a.node, null).view)
            isHeader(a.node) -> attachHeader(createView(a.node, rootKey?.let { entries[it]?.style }).view)
            isFooter(a.node) -> attachFooter(createView(a.node, rootKey?.let { entries[it]?.style }).view)
            oldWasLayer || oldWasHeader || oldWasFooter -> {
                // a layer that stops being one goes back to the end of the root, a header to the start
                val rootEntry = rootKey?.let { entries[it] } ?: return
                val container = rootEntry.container ?: return
                val created = createView(a.node, rootEntry.style)
                if (oldWasHeader) container.addView(created.view, 0) else container.addView(created.view)
                applyPercents(created.view, created.style, container)
            }
            else -> {
                val created = createView(a.node, parentEntry?.style)
                parent.addView(created.view, minOf(index, parent.childCount))
                applyPercents(created.view, created.style, parent)
            }
        }
    }

    private fun updateText(a: Change.UpdateText) {
        val entry = entries[a.key] ?: return
        (entry.view as? TextView)?.text = buildText(a.spans)
    }

    private fun updateStyle(a: Change.UpdateStyle) {
        val entry = entries[a.key] ?: return
        entry.style = a.style
        applyStyle(entry)
        if (isFooter(entry.node)) {
            // the dock or the size may have changed: the footer goes back to its place
            (entry.view.parent as? ViewGroup)?.removeView(entry.view)
            attachFooter(entry.view)
        }
        entry.list?.setStyle(a.style)
        // the icons are painted with the text color
        (entry.view as? ImageView)?.let { iv ->
            val n = entry.node
            if (n is NativeImage && IconSet.nameOf(n.source) != null) iv.setColorFilter(a.style.textColor)
        }

        val parent = entry.view.parent as? ViewGroup
        val parentEntry = entryOfParent(parent)
        if (parentEntry != null && parent != null) {
            entry.view.layoutParams = paramsFor(a.style, parentEntry.style)
            if (a.style.position != Position.ABSOLUTE) applyPercents(entry.view, a.style, parent)
        } else if (a.key == rootKey) {
            entry.view.layoutParams = rootParams(a.style)
            column?.setBackgroundColor(a.style.backgroundColor)
        } else if (entry.view.parent === headerHolder) {
            entry.view.layoutParams = rootParams(a.style)
        }

        val container = entry.container
        if (container != null) {
            for (i in 0 until container.childCount) {
                val child = container.getChildAt(i)
                val childEntry = (child.tag as? String)?.let { entries[it] } ?: continue
                child.layoutParams = childParams(childEntry.style, a.style)
                applyPercents(child, childEntry.style, container)
            }
        }
        (entry.view as? AbsFrame)?.let { frame ->
            for (i in 1 until frame.childCount) {
                val child = frame.getChildAt(i)
                val childEntry = (child.tag as? String)?.let { entries[it] } ?: continue
                child.layoutParams = absParams(childEntry.style)
            }
        }
    }

    private fun updateAttributes(a: Change.UpdateAttributes) {
        val entry = entries[a.key] ?: return
        entry.node = a.node
        when (val n = a.node) {
            is NativeButton -> (entry.view as? Button)?.let { configureButton(it, n) }
            is NativeField -> (entry.view as? EditText)?.let { configureField(it, n) }
            is NativeCheckbox -> (entry.view as? CheckBox)?.let { it.isEnabled = !n.disabled }
            is NativeImage -> (entry.view as? ImageView)?.let { configureImage(it, n) }
            is NativeProgress -> (entry.view as? NLProgress)?.setProgress(n.value, n.max, n.circular, animate = true)
            is NativeItem -> ((entry.view as? ViewGroup)?.getChildAt(0) as? TextView)?.text = n.marker + " "
            else -> {}
        }
        applyAlpha(entry)
        bindEvents(entry)
    }

    private fun entryOfParent(parent: ViewGroup?): ViewEntry? =
        (parent?.tag as? String)?.let { entries[it] }

    private fun discard(v: View) {
        (v.tag as? String)?.let { entries.remove(it) }
        if (v is ViewGroup) for (i in 0 until v.childCount) discard(v.getChildAt(i))
    }

    private fun createView(node: Native, parent: NativeStyle?): ViewEntry {
        val entry: ViewEntry = when (node) {
            is NativeBox -> {
                if (node.virtual) {
                    val vl = VirtualList(context)
                    ViewEntry(vl, null, node.style, node, vl)
                } else if (node.hasAbsolute) {
                    // the box has absolute children: a frame holds the flow and them
                    val frame = AbsFrame(context)
                    ViewEntry(frame, frame.flow, node.style, node)
                } else if (node.style.scrollY) {
                    // overflow-y: auto|scroll: a scroll holds the box that has the children
                    val scroll = NLScroll(context)
                    ViewEntry(scroll, scroll.content, node.style, node)
                } else {
                    val box = NLBox(context)
                    ViewEntry(box, box, node.style, node)
                }
            }
            is NativeList -> {
                val list = NLBox(context)
                ViewEntry(list, list, node.style, node)
            }
            is NativeItem -> {
                val row = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL }
                val markerView = TextView(context).apply { text = node.marker + " " }
                val content = NLBox(context)
                row.addView(markerView, LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT
                ))
                row.addView(content, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
                ViewEntry(row, content, node.style, node)
            }
            is NativeText -> {
                val tv = TextView(context)
                tv.text = buildText(node.spans)
                ViewEntry(tv, null, node.style, node)
            }
            is NativeButton -> {
                val b = Button(context)
                b.isAllCaps = false
                b.minHeight = 0
                b.minWidth = 0
                b.minimumHeight = 0
                b.minimumWidth = 0
                if (Build.VERSION.SDK_INT >= 21) b.stateListAnimator = null
                configureButton(b, node)
                ViewEntry(b, null, node.style, node)
            }
            is NativeField -> {
                val et = EditText(context)
                et.setText(node.value)
                configureField(et, node)
                ViewEntry(et, null, node.style, node)
            }
            is NativeCheckbox -> {
                val cb = CheckBox(context)
                cb.isChecked = node.checked
                cb.isEnabled = !node.disabled
                ViewEntry(cb, null, node.style, node)
            }
            is NativeProgress -> {
                val pv = NLProgress(context)
                pv.setProgress(node.value, node.max, node.circular, animate = false)
                ViewEntry(pv, null, node.style, node)
            }
            is NativeImage -> {
                val iv = ImageView(context)
                iv.adjustViewBounds = true
                iv.scaleType = ImageView.ScaleType.FIT_CENTER
                configureImage(iv, node)
                ViewEntry(iv, null, node.style, node)
            }
        }

        entry.view.tag = node.key
        if (entry.container != null && entry.container !== entry.view) {
            entry.container.tag = node.key
        }
        entries[node.key] = entry

        applyStyle(entry)
        if (parent != null) entry.view.layoutParams = paramsFor(node.style, parent)
        bindEvents(entry)

        entry.list?.let { it.setStyle(node.style); it.setModel(node.nativeChildren()) }

        val container = entry.container
        if (container != null) {
            for (child in node.nativeChildren()) {
                val childEntry = createView(child, node.style)
                if (isLayer(child)) {
                    attachLayer(childEntry.view)
                } else if (isHeader(child)) {
                    attachHeader(childEntry.view)
                } else if (isFooter(child)) {
                    attachFooter(childEntry.view)
                } else if (child.style.position == Position.ABSOLUTE && entry.view is AbsFrame) {
                    entry.view.addAbs(childEntry.view)
                } else {
                    container.addView(childEntry.view)
                    applyPercents(childEntry.view, childEntry.style, container)
                }
            }
        }
        return entry
    }

    private fun bindEvents(entry: ViewEntry) {
        val node = entry.node
        val v = entry.view
        if (v is EditText) return
        val interactive = node.id != null || node.classes.isNotEmpty()
        if (interactive) {
            val key = node.key
            v.setOnClickListener { publish(key) }
        } else {
            v.setOnClickListener(null)
            // a layer takes the taps, so that nothing under it is touched
            v.isClickable = v is Button || v is CheckBox || isLayer(node)
        }
    }

    private fun configureButton(b: Button, node: NativeButton) {
        b.text = node.label
        b.isEnabled = !node.disabled
    }

    private fun configureField(et: EditText, node: NativeField) {
        et.hint = node.hint
        val wantedInput = when (node.type) {
            FieldType.TEXT -> InputType.TYPE_CLASS_TEXT
            FieldType.PASSWORD -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            FieldType.NUMBER ->
                InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL or InputType.TYPE_NUMBER_FLAG_SIGNED
            FieldType.AREA -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
        }
        if (et.inputType != wantedInput) et.inputType = wantedInput
        if (node.type == FieldType.AREA) {
            et.minLines = maxOf(1, node.rows)
            et.gravity = Gravity.TOP or Gravity.START
        }
        et.isEnabled = !node.disabled
        et.isFocusable = !node.readOnly
        et.isFocusableInTouchMode = !node.readOnly
        et.isCursorVisible = !node.readOnly
    }

    private fun configureImage(iv: ImageView, node: NativeImage) {
        iv.contentDescription = node.description
        val icon = IconSet.nameOf(node.source)?.let { IconSet.shapesOf(it) }
        if (icon != null) {
            val size = (IconSet.GRID * context.resources.displayMetrics.density).toInt()
            iv.setImageDrawable(IconDrawable(icon, size))
            iv.setColorFilter(node.style.textColor)
            return
        }
        iv.clearColorFilter()
        val bitmap = loadImage(node.source)
        if (bitmap != null) {
            iv.setImageBitmap(bitmap)
            iv.setBackgroundColor(Color.TRANSPARENT)
        } else {
            iv.setImageDrawable(null)
            iv.setBackgroundColor(0xFFDDDDDD.toInt())
        }
    }

    private fun loadImage(source: String): Bitmap? {
        if (source.startsWith("data:")) {
            return try {
                val bytes = Base64.decode(source.substringAfter(','), Base64.DEFAULT)
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            } catch (e: Exception) {
                null
            }
        }
        return try {
            imageResolver(source)
        } catch (e: Exception) {
            null
        }
    }

    private fun buildText(spans: List<Span>): CharSequence {
        val sb = SpannableStringBuilder()
        for (t in spans) {
            val start = sb.length
            sb.append(t.text)
            val end = sb.length
            if (end > start) {
                sb.setSpan(ForegroundColorSpan(t.color), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                sb.setSpan(AbsoluteSizeSpan(pxF(t.size).roundToInt(), false), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                if (t.bold) sb.setSpan(StyleSpan(Typeface.BOLD), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                if (t.italic) sb.setSpan(StyleSpan(Typeface.ITALIC), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                if (t.underline) sb.setSpan(UnderlineSpan(), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
                if (t.strike) sb.setSpan(StrikethroughSpan(), start, end, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            }
        }
        return sb
    }

    private fun isDisabled(n: Native): Boolean = when (n) {
        is NativeButton -> n.disabled
        is NativeField -> n.disabled
        is NativeCheckbox -> n.disabled
        else -> false
    }

    /** opacity, times the disabled look (0.5 unless :disabled gives its own opacity). */
    private fun applyAlpha(entry: ViewEntry) {
        val st = entry.style
        var a = st.opacity
        if (isDisabled(entry.node)) a *= st.states?.disabled?.opacity ?: 0.5f
        entry.view.alpha = a
    }

    private fun typefaceOf(f: FontFamily): Typeface? = when (f) {
        FontFamily.DEFAULT -> null
        FontFamily.SANS -> Typeface.SANS_SERIF
        FontFamily.SERIF -> Typeface.SERIF
        FontFamily.MONOSPACE -> Typeface.MONOSPACE
    }

    private fun textColorsOf(st: NativeStyle): ColorStateList {
        val pressed = st.states?.pressed?.textColor
        val disabled = st.states?.disabled?.textColor
        val focused = st.states?.focused?.textColor
        if (pressed == null && disabled == null && focused == null) return ColorStateList.valueOf(st.textColor)
        return ColorStateList(
            arrayOf(
                intArrayOf(-android.R.attr.state_enabled),
                intArrayOf(android.R.attr.state_pressed),
                intArrayOf(android.R.attr.state_focused),
                intArrayOf()
            ),
            intArrayOf(disabled ?: st.textColor, pressed ?: st.textColor, focused ?: st.textColor, st.textColor)
        )
    }

    private fun applyText(v: TextView, st: NativeStyle, n: Native) {
        v.setTextColor(textColorsOf(st))
        v.setTextSize(TypedValue.COMPLEX_UNIT_PX, pxF(st.fontSize))
        val style = when {
            st.bold && st.italic -> Typeface.BOLD_ITALIC
            st.bold -> Typeface.BOLD
            st.italic -> Typeface.ITALIC
            else -> Typeface.NORMAL
        }
        v.setTypeface(typefaceOf(st.fontFamily), style)

        val sizePx = pxF(st.fontSize)
        v.letterSpacing = if (st.letterSpacing.value != 0f && sizePx > 0f) pxF(st.letterSpacing) / sizePx else 0f

        val natural = v.paint.getFontMetricsInt(null)
        when (val lh = st.lineHeight) {
            null -> v.setLineSpacing(0f, 1f)
            is LineHeight.Multiple -> v.setLineSpacing(lh.value * sizePx - natural, 1f)
            is LineHeight.Fixed -> v.setLineSpacing(pxF(lh.length) - natural, 1f)
        }

        if (n !is NativeText) {
            var flags = v.paintFlags and (Paint.UNDERLINE_TEXT_FLAG or Paint.STRIKE_THRU_TEXT_FLAG).inv()
            if (st.underline) flags = flags or Paint.UNDERLINE_TEXT_FLAG
            if (st.strike) flags = flags or Paint.STRIKE_THRU_TEXT_FLAG
            v.paintFlags = flags
        }

        when (n) {
            is NativeText -> v.gravity = textGravity(st)
            is NativeButton -> v.gravity = buttonGravity(st)
            else -> {}
        }
        if (v is EditText) {
            v.setHintTextColor((st.textColor and 0x00FFFFFF) or 0x80000000.toInt())
        }
        v.maxWidth = st.maxWidth?.let { px(it) } ?: Int.MAX_VALUE
        v.maxHeight = st.maxHeight?.let { px(it) } ?: Int.MAX_VALUE
    }

    private fun applyStyle(entry: ViewEntry) {
        val v = entry.view
        val st = entry.style
        val n = entry.node

        // the ring of a circular <progress> draws its own track: the view has no background
        applyBackground(
            v, if (n is NativeProgress && n.circular) st.copy(backgroundColor = 0, gradient = null, backgroundImage = null) else st,
            ripple = v is Button
        )
        // with absolute children the padding belongs to the flow: they are placed from the edge of the box;
        // in a scroll it belongs to the content, so it scrolls with it
        (when (v) { is AbsFrame -> v.flow; is NLScroll -> v.content; else -> v })
            .setPadding(px(st.padding.left), px(st.padding.top), px(st.padding.right), px(st.padding.bottom))
        applyAlpha(entry)
        v.minimumWidth = st.minWidth?.let { px(it) } ?: 0
        v.minimumHeight = st.minHeight?.let { px(it) } ?: 0

        if (v is TextView) applyText(v, st, n)

        if (v is ImageView) {
            if (n is NativeImage && IconSet.nameOf(n.source) == null) {
                v.scaleType = when (st.objectFit) {
                    ImageFit.COVER -> ImageView.ScaleType.CENTER_CROP
                    ImageFit.FILL -> ImageView.ScaleType.FIT_XY
                    ImageFit.CONTAIN -> ImageView.ScaleType.FIT_CENTER
                }
            }
            v.maxWidth = st.maxWidth?.let { px(it) } ?: Int.MAX_VALUE
            v.maxHeight = st.maxHeight?.let { px(it) } ?: Int.MAX_VALUE
        }

        if (v is CheckBox) {
            v.buttonTintList = ColorStateList(
                arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                intArrayOf(0xFF2F81F7.toInt(), Colors.BORDER_GRAY)
            )
        }

        if (n is NativeItem) {
            ((v as? ViewGroup)?.getChildAt(0) as? TextView)?.apply {
                setTextColor(st.textColor)
                setTextSize(TypedValue.COMPLEX_UNIT_PX, pxF(st.fontSize))
            }
        }

        val container = entry.container
        if (container is LinearLayout) applyContainer(container, st)
        if (v is NLScroll) {
            // the size limits are the ones of the scroll, not of the box inside it
            v.maxWidthPx = st.maxWidth?.let { px(it) } ?: -1
            v.maxHeightPx = st.maxHeight?.let { px(it) } ?: -1
            v.content.maxWidthPx = -1
            v.content.maxHeightPx = -1
            v.requestLayout()
        }
        if (v is NLProgress) {
            v.barColor = st.textColor
            v.trackColor = st.backgroundColor
            v.radiusPx = pxF(st.borderRadius)
            v.invalidate()
        }

        // overflow: hidden clips to the rounded corners; box-shadow becomes elevation
        if (st.overflowHidden) {
            val radius = pxF(st.borderRadius)
            val radii = st.corners?.let { cornerRadii(it) }
            v.outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) {
                    if (radii == null) {
                        outline.setRoundRect(0, 0, view.width, view.height, radius)
                    } else {
                        val path = Path()
                        path.addRoundRect(RectF(0f, 0f, view.width.toFloat(), view.height.toFloat()), radii, Path.Direction.CW)
                        outline.setConvexPath(path)
                    }
                }
            }
            v.clipToOutline = true
        } else {
            v.outlineProvider = ViewOutlineProvider.BACKGROUND
            v.clipToOutline = false
        }
        v.elevation = pxF(st.elevation)
    }

    private fun gradientOrientation(angle: Int): GradientDrawable.Orientation {
        val a = (((angle % 360) + 360) % 360 + 22) / 45 * 45 % 360
        return when (a) {
            0 -> GradientDrawable.Orientation.BOTTOM_TOP
            45 -> GradientDrawable.Orientation.BL_TR
            90 -> GradientDrawable.Orientation.LEFT_RIGHT
            135 -> GradientDrawable.Orientation.TL_BR
            180 -> GradientDrawable.Orientation.TOP_BOTTOM
            225 -> GradientDrawable.Orientation.TR_BL
            270 -> GradientDrawable.Orientation.RIGHT_LEFT
            else -> GradientDrawable.Orientation.BR_TL
        }
    }

    /** The shape of the element. [background] and [border] override the colors (state styles). */
    private fun shapeOf(e: NativeStyle, background: Int?, border: Int?): GradientDrawable {
        val g = if (background == null) e.gradient else null
        val shape = if (g != null) {
            GradientDrawable(gradientOrientation(g.angle), g.colors.toIntArray())
        } else {
            GradientDrawable().apply {
                setColor(background ?: (if (e.backgroundColor != 0) e.backgroundColor else Color.TRANSPARENT))
            }
        }
        val corners = e.corners
        if (corners != null) shape.setCornerRadii(cornerRadii(corners)) else shape.cornerRadius = pxF(e.borderRadius)
        if (e.borderWidth.value > 0f) shape.setStroke(maxOf(1, px(e.borderWidth)), border ?: e.borderColor)
        return shape
    }

    /** The eight values Android wants: x and y of top-left, top-right, bottom-right and bottom-left. */
    private fun cornerRadii(c: Corners): FloatArray {
        val tl = pxF(c.topLeft)
        val tr = pxF(c.topRight)
        val br = pxF(c.bottomRight)
        val bl = pxF(c.bottomLeft)
        return floatArrayOf(tl, tl, tr, tr, br, br, bl, bl)
    }

    private fun changesLook(s: StateStyle?): Boolean =
        s != null && (s.backgroundColor != null || s.borderColor != null)

    private fun applyBackground(view: View, e: NativeStyle, ripple: Boolean) {
        val pressed = e.states?.pressed
        val disabled = e.states?.disabled
        val focused = e.states?.focused
        val hasStates = changesLook(pressed) || changesLook(disabled) || changesLook(focused)
        val image = e.backgroundImage?.let { bitmapFor(it) }
        val hasBackground = e.backgroundColor != 0 || e.gradient != null || image != null
        val hasBorder = e.borderWidth.value > 0f
        if (!hasBackground && !hasBorder && !ripple && !hasStates) {
            view.background = null
            return
        }
        val base = shapeOf(e, null, null)
        val content: Drawable = if (hasStates) {
            val list = StateListDrawable()
            if (changesLook(disabled)) {
                list.addState(intArrayOf(-android.R.attr.state_enabled), shapeOf(e, disabled?.backgroundColor, disabled?.borderColor))
            }
            if (changesLook(pressed)) {
                list.addState(intArrayOf(android.R.attr.state_pressed), shapeOf(e, pressed?.backgroundColor, pressed?.borderColor))
            }
            if (changesLook(focused)) {
                list.addState(intArrayOf(android.R.attr.state_focused), shapeOf(e, focused?.backgroundColor, focused?.borderColor))
            }
            list.addState(intArrayOf(), base)
            list
        } else {
            base
        }
        val shown: Drawable = if (image != null) {
            val radii = e.corners?.let { cornerRadii(it) }
            LayerDrawable(arrayOf(content, ImageBackground(image, e.backgroundSize, e.backgroundPosition, pxF(e.borderRadius), radii)))
        } else {
            content
        }
        view.background = if (ripple && Build.VERSION.SDK_INT >= 21) {
            RippleDrawable(ColorStateList.valueOf(0x33000000), shown, null)
        } else {
            shown
        }
    }

    private val backgroundBitmaps = HashMap<String, Bitmap>()

    /** The image of a `background-image`, found as the source of an <img> is. Kept, so a restyle does not decode it again. */
    private fun bitmapFor(name: String): Bitmap? {
        backgroundBitmaps[name]?.let { return it }
        val bitmap = loadImage(name) ?: return null
        backgroundBitmaps[name] = bitmap
        return bitmap
    }

    private fun applyContainer(lin: LinearLayout, e: NativeStyle) {
        val vertical = e.direction == Direction.COLUMN
        lin.orientation = if (vertical) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
        lin.gravity = when (e.justify) {
            Justify.START, Justify.BETWEEN, Justify.AROUND, Justify.EVENLY ->
                if (vertical) Gravity.TOP else Gravity.START
            Justify.CENTER -> if (vertical) Gravity.CENTER_VERTICAL else Gravity.CENTER_HORIZONTAL
            Justify.END -> if (vertical) Gravity.BOTTOM else Gravity.END
        }
        val gap = px(e.gap)
        val wrapped = e.wrap && !vertical
        if (gap > 0 && !wrapped) {
            lin.dividerDrawable = GradientDrawable().apply {
                setSize(gap, gap)
                setColor(Color.TRANSPARENT)
            }
            lin.showDividers = LinearLayout.SHOW_DIVIDER_MIDDLE
        } else {
            lin.showDividers = LinearLayout.SHOW_DIVIDER_NONE
            lin.dividerDrawable = null
        }
        if (lin is NLBox) {
            lin.maxWidthPx = e.maxWidth?.let { px(it) } ?: -1
            lin.maxHeightPx = e.maxHeight?.let { px(it) } ?: -1
            lin.justify = e.justify
            lin.wrap = e.wrap
            lin.gapPx = gap
            lin.requestLayout()
        }
    }

    private fun buttonGravity(e: NativeStyle): Int = when (e.textAlign) {
        TextAlign.CENTER -> Gravity.CENTER
        TextAlign.END -> Gravity.END or Gravity.CENTER_VERTICAL
        TextAlign.START -> Gravity.START or Gravity.CENTER_VERTICAL
    }

    private fun textGravity(e: NativeStyle): Int {
        fun horiz(a: Align) = when (a) {
            Align.CENTER -> Gravity.CENTER_HORIZONTAL
            Align.END -> Gravity.END
            else -> Gravity.START
        }
        fun vert(a: Align) = when (a) {
            Align.CENTER -> Gravity.CENTER_VERTICAL
            Align.END -> Gravity.BOTTOM
            else -> Gravity.TOP
        }
        fun justHoriz(j: Justify) = when (j) {
            Justify.CENTER -> Gravity.CENTER_HORIZONTAL
            Justify.END -> Gravity.END
            else -> Gravity.START
        }
        fun justVert(j: Justify) = when (j) {
            Justify.CENTER -> Gravity.CENTER_VERTICAL
            Justify.END -> Gravity.BOTTOM
            else -> Gravity.TOP
        }

        var h = when (e.textAlign) {
            TextAlign.CENTER -> Gravity.CENTER_HORIZONTAL
            TextAlign.END -> Gravity.END
            TextAlign.START -> Gravity.START
        }
        val v: Int
        if (e.direction == Direction.COLUMN) {
            if (e.alignItems != Align.STRETCH && e.textAlign == TextAlign.START) h = horiz(e.alignItems)
            v = justVert(e.justify)
        } else {
            if (e.justify != Justify.START) h = justHoriz(e.justify)
            v = vert(e.alignItems)
        }
        return h or v
    }

    private fun rootParams(e: NativeStyle): FrameLayout.LayoutParams {
        val width = when (val m = e.width) {
            is Size.Dim -> px(m.length)
            else -> ViewGroup.LayoutParams.MATCH_PARENT
        }
        val h = e.height
        val height = when {
            e.flex > 0f -> ViewGroup.LayoutParams.MATCH_PARENT
            h is Size.Dim -> px(h.length)
            h is Size.Percent && h.value >= 100f -> ViewGroup.LayoutParams.MATCH_PARENT
            else -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        return FrameLayout.LayoutParams(width, height).apply {
            setMargins(px(e.margin.left), px(e.margin.top), px(e.margin.right), px(e.margin.bottom))
        }
    }

    private fun paramsFor(e: NativeStyle, parent: NativeStyle): ViewGroup.LayoutParams =
        if (e.position == Position.ABSOLUTE) absParams(e) else childParams(e, parent)

    /**
     * Place of an absolute child inside the frame of its box: left, top, right and bottom become
     * margins from the edge they name. With left and right (or top and bottom) and no size, it stretches.
     */
    private fun absParams(e: NativeStyle): AbsFrame.Lp {
        val l = e.left?.let { px(it) }
        val r = e.right?.let { px(it) }
        val t = e.top?.let { px(it) }
        val b = e.bottom?.let { px(it) }
        val w = e.width
        val h = e.height
        val width = when {
            w is Size.Dim -> px(w.length)
            w is Size.Percent && w.value >= 100f -> ViewGroup.LayoutParams.MATCH_PARENT
            l != null && r != null -> ViewGroup.LayoutParams.MATCH_PARENT
            else -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        val height = when {
            h is Size.Dim -> px(h.length)
            h is Size.Percent && h.value >= 100f -> ViewGroup.LayoutParams.MATCH_PARENT
            t != null && b != null -> ViewGroup.LayoutParams.MATCH_PARENT
            else -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        return AbsFrame.Lp(width, height).apply {
            gravity = (if (r != null && l == null) Gravity.END else Gravity.START) or
                (if (b != null && t == null) Gravity.BOTTOM else Gravity.TOP)
            setMargins(
                px(e.margin.left) + (l ?: 0), px(e.margin.top) + (t ?: 0),
                px(e.margin.right) + (r ?: 0), px(e.margin.bottom) + (b ?: 0)
            )
            z = e.zIndex
        }
    }

    private fun childParams(e: NativeStyle, parent: NativeStyle): LinearLayout.LayoutParams {
        val parentVertical = parent.direction == Direction.COLUMN
        val align = e.alignSelf ?: parent.alignItems

        fun mainSize(m: Size): Int = when (m) {
            is Size.Dim -> px(m.length)
            is Size.Percent ->
                if (m.value >= 100f) ViewGroup.LayoutParams.MATCH_PARENT else ViewGroup.LayoutParams.WRAP_CONTENT
            Size.Auto -> ViewGroup.LayoutParams.WRAP_CONTENT
        }
        fun crossSize(m: Size): Int = when (m) {
            is Size.Dim -> px(m.length)
            is Size.Percent ->
                if (m.value >= 100f) ViewGroup.LayoutParams.MATCH_PARENT else ViewGroup.LayoutParams.WRAP_CONTENT
            Size.Auto ->
                if (align == Align.STRETCH) ViewGroup.LayoutParams.MATCH_PARENT
                else ViewGroup.LayoutParams.WRAP_CONTENT
        }

        var width: Int
        var height: Int
        var weight = 0f
        if (parentVertical) {
            width = crossSize(e.width)
            height = mainSize(e.height)
            if (e.flex > 0f) { height = 0; weight = e.flex }
        } else {
            height = crossSize(e.height)
            width = mainSize(e.width)
            if (e.flex > 0f) { width = 0; weight = e.flex }
        }

        val lp = LinearLayout.LayoutParams(width, height, weight)
        lp.setMargins(px(e.margin.left), px(e.margin.top), px(e.margin.right), px(e.margin.bottom))
        lp.gravity = when (align) {
            Align.START -> if (parentVertical) Gravity.START else Gravity.TOP
            Align.CENTER -> if (parentVertical) Gravity.CENTER_HORIZONTAL else Gravity.CENTER_VERTICAL
            Align.END -> if (parentVertical) Gravity.END else Gravity.BOTTOM
            Align.STRETCH -> -1
        }
        return lp
    }

    private fun applyPercents(view: View, e: NativeStyle, parent: ViewGroup) {
        val l = (e.width as? Size.Percent)?.takeIf { it.value < 100f }
        val a = (e.height as? Size.Percent)?.takeIf { it.value < 100f }
        if (l == null && a == null) return

        fun apply(): Boolean {
            val lp = view.layoutParams ?: return true
            var pronto = true
            var mudou = false
            if (l != null) {
                val disponivel = parent.width - parent.paddingLeft - parent.paddingRight
                if (disponivel > 0) {
                    val alvo = (disponivel * l.value / 100f).roundToInt()
                    if (lp.width != alvo) { lp.width = alvo; mudou = true }
                } else {
                    pronto = false
                }
            }
            if (a != null) {
                val disponivel = parent.height - parent.paddingTop - parent.paddingBottom
                if (disponivel > 0) {
                    val alvo = (disponivel * a.value / 100f).roundToInt()
                    if (lp.height != alvo) { lp.height = alvo; mudou = true }
                } else {
                    pronto = false
                }
            }
            if (mudou) view.layoutParams = lp
            return pronto
        }

        parent.post {
            if (!apply()) {
                parent.addOnLayoutChangeListener(object : View.OnLayoutChangeListener {
                    override fun onLayoutChange(
                        v: View?, left: Int, top: Int, right: Int, bottom: Int,
                        oldLeft: Int, oldTop: Int, oldRight: Int, oldBottom: Int
                    ) {
                        if (apply()) parent.removeOnLayoutChangeListener(this)
                    }
                })
            }
        }
    }

    /**
     * Vertical list that only keeps views for the children near the visible area.
     * Heights of children never realized are estimated from the ones already measured.
     */
    private inner class VirtualList(ctx: Context) : ViewGroup(ctx) {
        private var items: List<Native> = emptyList()
        private var heights = IntArray(0)
        private var style: NativeStyle = NativeStyle()
        private val realized = HashMap<Int, ViewEntry>()
        private var sumKnown = 0L
        private var countKnown = 0
        private var refreshPosted = false

        private val scrollListener = ViewTreeObserver.OnScrollChangedListener { refresh() }
        private val refreshRunnable = Runnable { refreshPosted = false; refresh() }

        private fun dp(v: Int): Int = (v * density).roundToInt()
        private fun gap(): Int = px(style.gap)
        private fun childStyle(): NativeStyle = style.copy(direction = Direction.COLUMN)

        private fun estimate(): Int = if (countKnown > 0) maxOf(1, (sumKnown / countKnown).toInt()) else dp(80)
        private fun heightOf(i: Int): Int = if (heights[i] >= 0) heights[i] else estimate()

        private fun recount() {
            sumKnown = 0L
            countKnown = 0
            for (h in heights) if (h >= 0) { sumKnown += h; countKnown++ }
        }

        private fun dropView(e: ViewEntry) {
            discard(e.view)
            removeView(e.view)
        }

        fun setStyle(st: NativeStyle) {
            style = st
            val cs = childStyle()
            for ((_, e) in realized) {
                e.view.layoutParams = childParams(e.style, cs)
                applyPercents(e.view, e.style, this)
            }
            requestLayout()
        }

        fun setModel(newItems: List<Native>) {
            val old = items
            val oldHeights = heights
            val newHeights = IntArray(newItems.size) { -1 }
            if (old.isNotEmpty()) {
                val byKey = HashMap<String, Int>(old.size * 2)
                for (i in old.indices) if (i < oldHeights.size && oldHeights[i] >= 0) byKey[old[i].key] = oldHeights[i]
                for (i in newItems.indices) newHeights[i] = byKey[newItems[i].key] ?: -1
            }

            val oldRealized = HashMap<String, ViewEntry>()
            for ((idx, e) in realized) if (idx < old.size) oldRealized[old[idx].key] = e
            realized.clear()
            for (i in newItems.indices) {
                val e = oldRealized[newItems[i].key] ?: continue
                if (e.node.sameAs(newItems[i])) {
                    e.node = newItems[i]
                    realized[i] = e
                    oldRealized.remove(newItems[i].key)
                }
            }
            for ((_, e) in oldRealized) dropView(e)

            items = newItems
            heights = newHeights
            recount()
            requestLayout()
            refresh()
        }

        fun refresh() {
            if (items.isEmpty()) {
                for ((_, e) in realized) dropView(e)
                realized.clear()
                return
            }
            val r = Rect()
            var top = 0
            var bottom = dp(600)
            if (width > 0 && getLocalVisibleRect(r)) {
                top = r.top
                bottom = r.bottom
            }
            val span = maxOf(bottom - top, dp(300))
            val lo = top - span
            val hi = bottom + span

            val gap = gap()
            var first = -1
            var last = -1
            var y = paddingTop
            for (i in items.indices) {
                val h = heightOf(i)
                if (first < 0 && y + h >= lo) first = i
                if (y <= hi) last = i else break
                y += h + gap
            }
            if (first < 0) first = items.size - 1
            if (last < first) last = first

            var changed = false
            val iter = realized.entries.iterator()
            while (iter.hasNext()) {
                val en = iter.next()
                if (en.key < first || en.key > last) {
                    dropView(en.value)
                    iter.remove()
                    changed = true
                }
            }
            val cs = childStyle()
            for (i in first..last) {
                if (realized.containsKey(i)) continue
                val e = createView(items[i], cs)
                addView(e.view, e.view.layoutParams)
                applyPercents(e.view, e.style, this)
                realized[i] = e
                changed = true
            }
            if (changed) requestLayout()
        }

        override fun onAttachedToWindow() {
            super.onAttachedToWindow()
            viewTreeObserver.addOnScrollChangedListener(scrollListener)
        }

        override fun onDetachedFromWindow() {
            viewTreeObserver.removeOnScrollChangedListener(scrollListener)
            removeCallbacks(refreshRunnable)
            refreshPosted = false
            super.onDetachedFromWindow()
        }

        override fun onMeasure(widthSpec: Int, heightSpec: Int) {
            val width = MeasureSpec.getSize(widthSpec)
            val avail = maxOf(0, width - paddingLeft - paddingRight)
            var measuredChanged = false
            for ((idx, e) in realized) {
                val v = e.view
                val lp = v.layoutParams as MarginLayoutParams
                val margins = lp.leftMargin + lp.rightMargin
                val cw = when {
                    lp.width == LayoutParams.MATCH_PARENT ->
                        MeasureSpec.makeMeasureSpec(maxOf(0, avail - margins), MeasureSpec.EXACTLY)
                    lp.width >= 0 -> MeasureSpec.makeMeasureSpec(lp.width, MeasureSpec.EXACTLY)
                    else -> MeasureSpec.makeMeasureSpec(maxOf(0, avail - margins), MeasureSpec.AT_MOST)
                }
                val ch = if (lp.height >= 0) MeasureSpec.makeMeasureSpec(lp.height, MeasureSpec.EXACTLY)
                else MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED)
                v.measure(cw, ch)
                val h = v.measuredHeight + lp.topMargin + lp.bottomMargin
                if (idx < heights.size && heights[idx] != h) {
                    heights[idx] = h
                    measuredChanged = true
                }
            }
            if (measuredChanged) recount()

            var total = paddingTop + paddingBottom
            for (i in items.indices) total += heightOf(i)
            if (items.size > 1) total += gap() * (items.size - 1)
            val height = if (MeasureSpec.getMode(heightSpec) == MeasureSpec.EXACTLY) MeasureSpec.getSize(heightSpec) else total
            setMeasuredDimension(width, height)
        }

        override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
            val gap = gap()
            var y = paddingTop
            var maxIdx = -1
            for (idx in realized.keys) if (idx > maxIdx) maxIdx = idx
            for (i in 0..maxIdx) {
                val h = heightOf(i)
                val e = realized[i]
                if (e != null) {
                    val v = e.view
                    val lp = v.layoutParams as MarginLayoutParams
                    val left = paddingLeft + lp.leftMargin
                    val topPos = y + lp.topMargin
                    v.layout(left, topPos, left + v.measuredWidth, topPos + v.measuredHeight)
                }
                y += h + gap
            }
            if (!refreshPosted) {
                refreshPosted = true
                post(refreshRunnable)
            }
        }

        override fun checkLayoutParams(p: LayoutParams?): Boolean = p is MarginLayoutParams

        override fun generateLayoutParams(p: LayoutParams): LayoutParams =
            if (p is MarginLayoutParams) p else MarginLayoutParams(p)

        override fun generateDefaultLayoutParams(): LayoutParams =
            MarginLayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT)

        override fun generateLayoutParams(attrs: AttributeSet?): LayoutParams = MarginLayoutParams(context, attrs)
    }
}
