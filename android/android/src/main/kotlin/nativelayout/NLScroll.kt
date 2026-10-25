/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-25
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout

import android.content.Context
import android.view.MotionEvent
import android.view.ViewGroup
import android.widget.ScrollView

/**
 * A box with `overflow-y: auto|scroll`: a vertical scroll whose only child is the [content] box
 * that holds the children of the element. max-width and max-height of the element limit the scroll
 * itself, so the content scrolls inside when it is taller than the space the box can have.
 */
internal class NLScroll(context: Context) : ScrollView(context) {
    val content = NLBox(context)

    var maxWidthPx: Int = -1
    var maxHeightPx: Int = -1

    init {
        isFillViewport = true
        // overflow is clipped by the scroll itself; the content is not drawn outside it
        clipToPadding = true
        addView(content, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
    }

    private fun clamp(spec: Int, max: Int): Int {
        if (max < 0) return spec
        val size = MeasureSpec.getSize(spec)
        return when (MeasureSpec.getMode(spec)) {
            MeasureSpec.EXACTLY ->
                if (size > max) MeasureSpec.makeMeasureSpec(max, MeasureSpec.EXACTLY) else spec
            MeasureSpec.AT_MOST -> MeasureSpec.makeMeasureSpec(minOf(size, max), MeasureSpec.AT_MOST)
            else -> MeasureSpec.makeMeasureSpec(max, MeasureSpec.AT_MOST)
        }
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        super.onMeasure(clamp(widthMeasureSpec, maxWidthPx), clamp(heightMeasureSpec, maxHeightPx))
    }

    /**
     * The screen scrolls too. While this box can scroll, the gesture that starts on it belongs to it,
     * so the screen does not take it away; a box with nothing to scroll leaves the gesture to the screen.
     */
    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN) {
            val scrollable = canScrollVertically(-1) || canScrollVertically(1)
            (parent as? ViewGroup)?.requestDisallowInterceptTouchEvent(scrollable)
        }
        return super.dispatchTouchEvent(ev)
    }
}