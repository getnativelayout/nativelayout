/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-25
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.view.View
import android.view.animation.LinearInterpolator
import android.view.animation.PathInterpolator
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/**
 * The view of a <progress>: a bar or, with `circular`, a ring.
 *
 * The track of the bar is the normal background of the view (so it can have a radius, a gradient and so on);
 * this view draws the filled part over it. The ring has no background: its track is drawn here.
 *
 * The motion is the one of framer-motion: a change of the value moves with the curve of its default
 * transition (cubic-bezier(.25, .1, .35, 1) in 0.3 s) and the indeterminate loop uses its `easeInOut`.
 */
internal class NLProgress(context: Context) : View(context) {
    /** Color of the filled part: the `color` of the element. */
    var barColor: Int = 0xFF2F81F7.toInt()

    /** Color of the track of the ring: the `background-color` of the element. The bar uses the view background. */
    var trackColor: Int = 0

    /** Radius of the corners of the bar, in pixels. It is cut to half of the height. */
    var radiusPx: Float = 0f

    private var value: Float? = null
    private var max: Float = 1f
    private var circular = false

    /** Done as shown now (0 to 1): it moves toward the real value. */
    private var shown = 0f

    /** Position in the loop of the indeterminate animation (0 to 1). */
    private var phase = 0f

    private var loop: ValueAnimator? = null
    private var fill: ValueAnimator? = null

    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val clip = Path()
    private val oval = RectF()

    private val indeterminate: Boolean get() = value == null

    fun setProgress(value: Float?, max: Float, circular: Boolean, animate: Boolean) {
        this.value = value
        this.max = max
        this.circular = circular
        val target = if (value == null) 0f else (value / max).coerceIn(0f, 1f)
        fill?.cancel()
        if (value == null) {
            shown = 0f
            startLoop()
        } else {
            stopLoop()
            if (animate && isAttachedToWindow && target != shown) {
                fill = ValueAnimator.ofFloat(shown, target).apply {
                    duration = FILL_MS
                    interpolator = EASE
                    addUpdateListener { shown = it.animatedValue as Float; invalidate() }
                    start()
                }
            } else {
                shown = target
            }
        }
        requestLayout()
        invalidate()
    }

    private fun startLoop() {
        if (loop != null || !isAttachedToWindow || !indeterminate) return
        loop = ValueAnimator.ofFloat(0f, 1f).apply {
            duration = LOOP_MS
            repeatCount = ValueAnimator.INFINITE
            interpolator = LinearInterpolator()
            addUpdateListener { phase = it.animatedValue as Float; invalidate() }
            start()
        }
    }

    private fun stopLoop() {
        loop?.cancel()
        loop = null
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        startLoop()
    }

    override fun onDetachedFromWindow() {
        stopLoop()
        fill?.cancel()
        super.onDetachedFromWindow()
    }

    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        val d = resources.displayMetrics.density
        val w = if (circular) 32f * d else 160f * d
        val h = if (circular) 32f * d else 8f * d
        setMeasuredDimension(
            resolveSize(w.toInt() + paddingLeft + paddingRight, widthMeasureSpec),
            resolveSize(h.toInt() + paddingTop + paddingBottom, heightMeasureSpec)
        )
    }

    override fun onDraw(canvas: Canvas) {
        if (circular) drawRing(canvas) else drawBar(canvas)
    }

    private fun drawBar(canvas: Canvas) {
        val w = width.toFloat()
        val h = height.toFloat()
        val r = min(radiusPx, h / 2f)
        clip.reset()
        clip.addRoundRect(0f, 0f, w, h, r, r, Path.Direction.CW)
        canvas.save()
        canvas.clipPath(clip)
        paint.style = Paint.Style.FILL
        paint.color = barColor
        if (indeterminate) {
            // a piece crosses the bar, slowing down at both ends
            val piece = w * PIECE
            val left = -piece + EASE_IN_OUT.getInterpolation(phase) * (w + piece)
            canvas.drawRect(left, 0f, left + piece, h, paint)
        } else {
            canvas.drawRect(0f, 0f, w * shown, h, paint)
        }
        canvas.restore()
    }

    private fun drawRing(canvas: Canvas) {
        val size = min(width, height).toFloat()
        val stroke = max(resources.displayMetrics.density * 2f, size / 10f)
        val cx = width / 2f
        val cy = height / 2f
        val half = size / 2f - stroke / 2f
        oval.set(cx - half, cy - half, cx + half, cy + half)
        paint.style = Paint.Style.STROKE
        paint.strokeWidth = stroke
        paint.strokeCap = Paint.Cap.ROUND
        if (trackColor != 0) {
            paint.color = trackColor
            canvas.drawOval(oval, paint)
        }
        paint.color = barColor
        if (indeterminate) {
            // the arc turns twice per loop and grows and shrinks between a short and a long piece
            val grow = EASE_IN_OUT.getInterpolation(1f - abs(2f * phase - 1f))
            canvas.drawArc(oval, -90f + phase * 720f, MIN_ARC + (MAX_ARC - MIN_ARC) * grow, false, paint)
        } else if (shown > 0f) {
            canvas.drawArc(oval, -90f, shown * 360f, false, paint)
        }
    }

    private companion object {
        const val LOOP_MS = 1400L
        const val FILL_MS = 300L
        const val PIECE = 0.4f
        const val MIN_ARC = 30f
        const val MAX_ARC = 270f

        /** Default transition of framer-motion for a value that is not a transform. */
        val EASE = PathInterpolator(0.25f, 0.1f, 0.35f, 1f)

        /** The `easeInOut` of framer-motion. */
        val EASE_IN_OUT = PathInterpolator(0.42f, 0f, 0.58f, 1f)
    }
}