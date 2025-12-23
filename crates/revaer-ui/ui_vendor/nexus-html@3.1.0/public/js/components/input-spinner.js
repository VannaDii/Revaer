// Input Spinner
class InputSpinner {
    constructor(input) {
        this.input = typeof input === "string" ? document.querySelector(input) : input

        if (!this.input || !this.input.id) {
            throw new Error("Input element with valid id required")
        }

        this.buttons = document.querySelectorAll(`[data-spinner-control="${this.input.id}"]`)
        this.holdInterval = null
        this.holdTimeout = null
        this.listeners = []

        this.init()
        this.validateButtons()
    }

    addListener(el, type, fn) {
        el.addEventListener(type, fn)
        this.listeners.push({ el, type, fn })
    }

    init() {
        this.buttons.forEach((btn) => {
            this.addListener(btn, "mousedown", () => this.startHold(btn))
            this.addListener(btn, "touchstart", () => this.startHold(btn))
            this.addListener(btn, "mouseup", () => this.stopHold())
            this.addListener(btn, "mouseleave", () => this.stopHold())
            this.addListener(btn, "touchend", () => this.stopHold())
            this.addListener(btn, "click", () => {
                this.handleSpin(btn)
                this.validateButtons()
            })
        })

        this.addListener(this.input, "input", () => this.validateButtons())
    }

    startHold(btn) {
        this.stopHold()

        this.holdTimeout = setTimeout(() => {
            this.holdInterval = setInterval(() => {
                this.handleSpin(btn)
                this.validateButtons()
            }, 100)
        }, 400)
    }

    stopHold() {
        if (this.holdTimeout) {
            clearTimeout(this.holdTimeout)
            this.holdTimeout = null
        }
        if (this.holdInterval) {
            clearInterval(this.holdInterval)
            this.holdInterval = null
        }
    }

    handleSpin(btn) {
        if (btn.hasAttribute("disabled")) return

        const rawStep = btn.getAttribute("data-steps") || "1"
        const step = parseFloat(rawStep)
        const min = this.input.hasAttribute("min") ? parseFloat(this.input.min) : -Infinity
        const max = this.input.hasAttribute("max") ? parseFloat(this.input.max) : Infinity
        const current = parseFloat(this.input.value) || 0

        let next = current + step
        next = Math.max(min, Math.min(max, next))
        const decimals = this.getMaxDecimals(current, step)

        this.input.value = next.toFixed(decimals)
        this.input.dispatchEvent(new Event("change"))
    }

    validateButtons() {
        const min = this.input.hasAttribute("min") ? parseFloat(this.input.min) : -Infinity
        const max = this.input.hasAttribute("max") ? parseFloat(this.input.max) : Infinity
        const current = parseFloat(this.input.value) || 0

        this.buttons.forEach((btn) => {
            const step = parseFloat(btn.getAttribute("data-steps") || "1")
            const disabled = (step < 0 && current <= min) || (step > 0 && current >= max)
            btn.toggleAttribute("disabled", disabled)
            btn.classList.toggle("disabled", disabled)
        })
    }

    getMaxDecimals(a, b) {
        const aDec = (a.toString().split(".")[1] || "").length
        const bDec = (b.toString().split(".")[1] || "").length
        return Math.max(aDec, bDec)
    }

    destroy() {
        this.stopHold()
        this.listeners.forEach(({ el, type, fn }) => el.removeEventListener(type, fn))
        this.listeners = []
    }
}

// Usages
document.querySelectorAll("[data-spinner]").forEach((el) => new InputSpinner(el))
