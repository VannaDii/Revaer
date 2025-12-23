window.addEventListener("DOMContentLoaded", () => {
    new Sortable(document.getElementById("in-action-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
    })

    new Sortable(document.getElementById("simple-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
    })

    new Sortable(document.getElementById("handle-sortable-demo"), {
        animation: 150,
        handle: "[data-handle]",
        ghostClass: "ghost",
        dragClass: "drag",
    })

    new Sortable(document.getElementById("shared-sortable-demo-1"), {
        animation: 150,
        group: "shared",
        ghostClass: "ghost",
        dragClass: "drag",
    })
    new Sortable(document.getElementById("shared-sortable-demo-2"), {
        animation: 150,
        group: "shared",
        ghostClass: "ghost",
        dragClass: "drag",
    })
    new Sortable(document.getElementById("filter-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
        filter: ".no-sort",
    })
    new Sortable(document.getElementById("animated-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
    })
    new Sortable(document.getElementById("multi-drag-sortable-demo"), {
        multiDrag: true,
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
        selectedClass: "selected",
    })
    new Sortable(document.getElementById("swap-sortable-demo"), {
        swap: true,
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
        swapClass: "p-swap",
    })
    new Sortable(document.getElementById("grid-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
    })
    new Sortable(document.getElementById("grid-handle-sortable-demo"), {
        animation: 150,
        ghostClass: "ghost",
        dragClass: "drag",
        handle: "[data-handle]",
    })
})
