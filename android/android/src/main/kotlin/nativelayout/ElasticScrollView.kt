/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-21
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout

import android.animation.ValueAnimator
import android.content.Context
import android.view.MotionEvent
import android.view.View
import android.view.animation.DecelerateInterpolator
import android.widget.ScrollView
import kotlin.math.abs
import kotlin.math.min

/**
 * A ScrollView that stretches like rubber at the ends, for Android before 12, which has no
 * stretch effect of its own. The content is stretched from the edge that is being pulled, with
 * resistance, and springs back when the finger leaves. It is not moved: it stretches.
 */
internal class ElasticScrollView(context: Context) : ScrollView(context) {
    private var lastY = 0f
    private var pull = 0f
    private var anim: ValueAnimator? = null

    init {
        overScrollMode = OVER_SCROLL_NEVER
    }

    /**
     * Sees every touch start, even when a child (a button, for example) consumes the DOWN and the
     * scroll view only gets the following events. Without this, `lastY` would be stale in the first
     * MOVE and the first `dy` would be huge.
     */
    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN) {
            anim?.cancel()
            lastY = ev.y
        }
        return super.dispatchTouchEvent(ev)
    }

    /**
     * The MOVE that starts the drag is taken here and never reaches [onTouchEvent], so its position
     * is recorded here too. Otherwise the first `dy` would include the distance of that move.
     */
    override fun onInterceptTouchEvent(ev: MotionEvent): Boolean {
        val intercept = super.onInterceptTouchEvent(ev)
        if (ev.actionMasked == MotionEvent.ACTION_MOVE) lastY = ev.y
        return intercept
    }

    override fun onTouchEvent(ev: MotionEvent): Boolean {
        val child = getChildAt(0) ?: return super.onTouchEvent(ev)
        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                anim?.cancel()
                lastY = ev.y
            }
            MotionEvent.ACTION_MOVE -> {
                val dy = ev.y - lastY
                lastY = ev.y
                val t = pull
                val pullDown = t > 0f || (dy > 0f && !canScrollVertically(-1))
                val pullUp = t < 0f || (dy < 0f && !canScrollVertically(1))
                if (pullDown || pullUp) {
                    val returning = t != 0f && dy * t < 0f
                    val resist = if (returning) 1f else 0.5f * (1f - min(abs(t) / height, 0.8f))
                    var next = t + dy * resist
                    if ((t > 0f && next < 0f) || (t < 0f && next > 0f)) next = 0f
                    apply(child, next)
                    // The scroll view did not see the moves of the stretch. When the stretch ends in the
                    // middle of the gesture, it is told that a new touch starts here, so that it scrolls
                    // from this point and not from where the finger first touched.
                    if (t != 0f && next == 0f) resync(ev)
                    return true
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> springBack(child)
        }
        return super.onTouchEvent(ev)
    }

    private fun resync(ev: MotionEvent) {
        val down = MotionEvent.obtain(ev)
        down.action = MotionEvent.ACTION_DOWN
        super.onTouchEvent(down)
        down.recycle()
    }

    /** [value] is how far the finger pulled: positive from the top, negative from the bottom. */
    private fun apply(child: View, value: Float) {
        pull = value
        if (value == 0f) {
            child.scaleY = 1f
            child.translationY = 0f
            return
        }
        // the edge that is pulled stays where it is and the rest stretches away from it
        child.pivotX = child.width / 2f
        child.pivotY = if (value > 0f) 0f else child.height.toFloat()
        val ratio = min(abs(value) / height, 1f)
        child.scaleY = 1f + MAX_STRETCH * ratio
        child.translationY = 0f
    }

    private fun springBack(child: View) {
        val from = pull
        if (from == 0f) return
        anim = ValueAnimator.ofFloat(from, 0f).apply {
            duration = 300
            interpolator = DecelerateInterpolator()
            addUpdateListener { apply(child, it.animatedValue as Float) }
            start()
        }
    }

    private companion object {
        /** Biggest stretch of the content when the pull is as long as the screen: 0.3 means 30% taller. */
        const val MAX_STRETCH = 0.3f
    }
}