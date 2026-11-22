/* ============================================================
   AUTOMATA — DRAWING TOOL
   Ek chhota paint canvas. Mouse / touch se draw karo, colour aur
   brush size badlo, PNG save karo. Koi network call nahi — sab
   local hai, is liye agent ke chalte hue bhi kaam karta hai.
   ============================================================ */

(function () {
  "use strict";

  const COLORS = ["#e9effc", "#f2b134", "#4a90d9", "#4fbf8b", "#e05c5c", "#b07cff"];
  const CANVAS_W = 1000;
  const CANVAS_H = 580;
  const BACKGROUND = "#0b1220";

  const state = {
    color: COLORS[0],
    size: 4,
    erasing: false,
    drawing: false,
    last: null
  };

  function $(id) { return document.getElementById(id); }

  function paintBackground(ctx) {
    ctx.save();
    ctx.globalCompositeOperation = "source-over";
    ctx.fillStyle = BACKGROUND;
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
    ctx.restore();
  }

  function init() {
    const overlay = $("draw-overlay");
    const canvas = $("draw-canvas");
    const openButton = $("btn-draw");
    if (!overlay || !canvas || !openButton) return;

    const ctx = canvas.getContext("2d");
    paintBackground(ctx);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";

    /* ---- Colour swatches ---- */
    const swatches = $("draw-swatches");
    COLORS.forEach(function (color, index) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "draw-swatch" + (index === 0 ? " is-active" : "");
      button.style.background = color;
      button.setAttribute("aria-label", "Brush colour " + color);
      button.addEventListener("click", function () {
        state.color = color;
        state.erasing = false;
        $("draw-eraser").setAttribute("aria-pressed", "false");
        swatches.querySelectorAll(".draw-swatch").forEach(function (item) {
          item.classList.remove("is-active");
        });
        button.classList.add("is-active");
      });
      swatches.appendChild(button);
    });

    /* ---- Brush size ---- */
    $("draw-size").addEventListener("input", function () {
      state.size = Number(this.value) || 4;
      $("draw-size-value").textContent = String(state.size);
    });

    /* ---- Eraser ---- */
    $("draw-eraser").addEventListener("click", function () {
      state.erasing = !state.erasing;
      this.setAttribute("aria-pressed", state.erasing ? "true" : "false");
    });

    /* ---- Clear ---- */
    $("draw-clear").addEventListener("click", function () {
      paintBackground(ctx);
    });

    /* ---- Save PNG ---- */
    $("draw-save").addEventListener("click", function () {
      const link = document.createElement("a");
      link.download = "automata-draw.png";
      link.href = canvas.toDataURL("image/png");
      document.body.appendChild(link);
      link.click();
      link.remove();
    });

    /* ---- Open / close ---- */
    function close() {
      overlay.classList.add("hidden");
      openButton.focus();
    }

    openButton.addEventListener("click", function () {
      overlay.classList.remove("hidden");
      canvas.focus();
    });
    $("draw-close").addEventListener("click", close);
    overlay.addEventListener("click", function (event) {
      if (event.target === overlay) close();
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && !overlay.classList.contains("hidden")) {
        close();
      }
    });

    /* ---- Drawing ---- */
    function point(event) {
      const rect = canvas.getBoundingClientRect();
      return {
        x: ((event.clientX - rect.left) / rect.width) * CANVAS_W,
        y: ((event.clientY - rect.top) / rect.height) * CANVAS_H
      };
    }

    function stroke(from, to) {
      ctx.save();
      ctx.strokeStyle = state.erasing ? BACKGROUND : state.color;
      ctx.fillStyle = state.erasing ? BACKGROUND : state.color;
      ctx.lineWidth = state.erasing ? state.size * 3 : state.size;

      if (from.x === to.x && from.y === to.y) {
        // Sirf click kiya: ek chhota dot banao.
        ctx.beginPath();
        ctx.arc(to.x, to.y, ctx.lineWidth / 2, 0, Math.PI * 2);
        ctx.fill();
      } else {
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
      }
      ctx.restore();
    }

    canvas.addEventListener("pointerdown", function (event) {
      canvas.setPointerCapture(event.pointerId);
      state.drawing = true;
      state.last = point(event);
      stroke(state.last, state.last);
    });

    canvas.addEventListener("pointermove", function (event) {
      if (!state.drawing) return;
      const next = point(event);
      stroke(state.last, next);
      state.last = next;
    });

    function endStroke(event) {
      if (!state.drawing) return;
      state.drawing = false;
      state.last = null;
      try { canvas.releasePointerCapture(event.pointerId); } catch (e) { /* already gone */ }
    }

    canvas.addEventListener("pointerup", endStroke);
    canvas.addEventListener("pointercancel", endStroke);
  }

  document.addEventListener("DOMContentLoaded", init);
})();
