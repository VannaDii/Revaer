class PasswordMeter {
    constructor(target) {
        this.input = typeof target === "string" ? document.querySelector(target) : target

        this.rules = [
            (v) => v.length >= 8,
            (v) => /[a-z]/.test(v),
            (v) => /[A-Z]/.test(v),
            (v) => /\d/.test(v),
            (v) => /\W/.test(v),
        ]

        // Bind update to the instance
        this.update = this.update.bind(this)

        this.input?.addEventListener("input", this.update)
    }

    update() {
        if (!this.input) return

        const val = this.input.value
        let passed = 0

        this.rules.forEach((rule, i) => {
            const attr = `data-pass-r${i + 1}`
            if (rule(val)) {
                this.input.setAttribute(attr, "")
                passed++
            } else {
                this.input.removeAttribute(attr)
            }
        })

        for (let i = 1; i <= this.rules.length; i++) {
            const attr = `data-pass-p${i * 20}`
            if (i <= passed) {
                this.input.setAttribute(attr, "")
            } else {
                this.input.removeAttribute(attr)
            }
        }
    }

    destroy() {
        if (!this.input) return
        this.input.removeEventListener("input", this.update)
        for (let i = 1; i <= this.rules.length; i++) {
            this.input.removeAttribute(`data-pass-r${i}`)
            this.input.removeAttribute(`data-pass-p${i * 20}`)
        }
    }
}

window.addEventListener("DOMContentLoaded", () => {
    new PasswordMeter("#both-password-meter-demo")

    new PasswordMeter("#progress-password-meter-demo")

    new PasswordMeter("#rules-password-meter-demo")
})
