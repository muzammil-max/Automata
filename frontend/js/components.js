/* ============================================================
   AUTOMATA — SMALL SHARED COMPONENTS
   - Logo (browser window + three cells + amber cursor)
   - openDialog (simple modern dialog)
   ============================================================ */

(function () {
  "use strict";

  const SVG_NS = "http://www.w3.org/2000/svg";

  /* ----------------------------------------------------------
     LOGO
     A browser window containing three cells in a triangle (an "A"),
     with the amber cursor pressing the bottom-right cell.
     ---------------------------------------------------------- */
  function Logo(size) {
    size = size || 48;
    const small = size <= 24;
    const stroke = small ? 4 : 2.5;
    const cell = small ? 9 : 7;

    const svg = document.createElementNS(SVG_NS, "svg");
    svg.setAttribute("viewBox", "0 0 48 40");
    svg.setAttribute("width", size);
    svg.setAttribute("height", Math.round(size * 40 / 48));
    svg.setAttribute("class", "logo");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");

    const body =
      '<rect x="' + (stroke / 2) + '" y="' + (stroke / 2) +
        '" width="' + (48 - stroke) + '" height="' + (40 - stroke) +
        '" fill="none" stroke="currentColor" stroke-width="' + stroke + '"></rect>' +
      '<line x1="' + (stroke / 2) + '" y1="11" x2="' + (48 - stroke / 2) +
        '" y2="11" stroke="currentColor" stroke-width="' + stroke + '"></line>' +
      '<rect x="' + (24 - cell / 2) + '" y="16" width="' + cell +
        '" height="' + cell + '" fill="currentColor"></rect>' +
      '<rect x="14" y="' + (16 + cell + 3) + '" width="' + cell +
        '" height="' + cell + '" fill="currentColor"></rect>' +
      '<rect x="' + (14 + cell + 6) + '" y="' + (16 + cell + 3) +
        '" width="' + cell + '" height="' + cell + '" fill="#F2B134"></rect>' +
      '<path d="M33 24 l0 12 l3 -3 l2.5 5 l2.5 -1.2 l-2.5 -5 l4.5 -0.5 z" fill="#F2B134"></path>';

    svg.innerHTML = body;
    return svg;
  }

  /* ----------------------------------------------------------
     DIALOG
     openDialog({ title, message, input, blocking, buttons })
     buttons: [{ label, primary, onClick, keepOpen }]
     ---------------------------------------------------------- */
  function openDialog(opts) {
    const root = document.getElementById("dialog-root");
    if (!root) return { close: function () {} };

    const previousFocus = document.activeElement;
    const backdrop = document.createElement("div");
    backdrop.className = "dialog-backdrop";

    const card = document.createElement("div");
    card.className = "dialog";
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");

    const title = document.createElement("h2");
    title.className = "dialog-title";
    title.textContent = opts.title || "Automata";
    card.appendChild(title);

    if (opts.message) {
      const msg = document.createElement("p");
      msg.className = "dialog-message";
      msg.textContent = opts.message;
      card.appendChild(msg);
    }

    let input = null;
    if (opts.input) {
      input = document.createElement("input");
      input.className = "dialog-input";
      input.type = "text";
      input.value = opts.input.value || "";
      if (opts.input.placeholder) input.placeholder = opts.input.placeholder;
      card.appendChild(input);
    }

    const actions = document.createElement("div");
    actions.className = "dialog-actions";

    function close() {
      backdrop.remove();
      document.removeEventListener("keydown", onKey);
      if (previousFocus && previousFocus.focus) previousFocus.focus();
    }

    function onKey(e) {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      }
      if (e.key === "Enter" && input) {
        e.preventDefault();
        const primary = actions.querySelector(".dialog-btn--primary") ||
                        actions.querySelector(".dialog-btn");
        if (primary) primary.click();
      }
    }

    (opts.buttons || [{ label: "OK", primary: true }]).forEach(function (b, index) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "dialog-btn" + (b.primary ? " dialog-btn--primary" : "");
      btn.textContent = b.label;
      btn.addEventListener("click", function () {
        if (!b.keepOpen) close();
        if (b.onClick) b.onClick(input ? input.value : undefined);
      });
      actions.appendChild(btn);
      if (index === 0) setTimeout(function () { btn.focus(); }, 0);
    });

    card.appendChild(actions);
    backdrop.appendChild(card);
    backdrop.addEventListener("click", function (e) {
      if (e.target === backdrop && !opts.blocking) close();
    });
    document.addEventListener("keydown", onKey);
    root.appendChild(backdrop);

    if (input) {
      setTimeout(function () { input.focus(); input.select(); }, 0);
    }

    return { close: close };
  }

  window.AutomataUI = {
    Logo: Logo,
    openDialog: openDialog
  };
})();
