window.addEventListener("DOMContentLoaded", () => {
    new Choices("#simple-select-demo")

    new Choices("#disabled-select-demo")

    new Choices("#group-select-demo")

    new Choices("#multiple-select-demo")

    new Choices("#removable-select-demo", { removeItemButton: true })

    new Choices("#searchable-select-demo", { searchEnabled: true, noChoicesText: "Hello" })

    new Choices("#text-input-select-demo")

    new Choices("#email-input-select-demo", {
        allowHTML: true,
        editItems: true,
        removeItemButton: true,
        addItemFilter: (value) => {
            if (!value) return false
            const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
            return new RegExp(regex.source, "i").test(value)
        },
    })
})
