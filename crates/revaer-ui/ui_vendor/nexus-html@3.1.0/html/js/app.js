class LayoutCustomizer {
    constructor() {
        this.defaultConfig = {
            theme: "system",
            direction: "ltr",
            fontFamily: "default",
            sidebarTheme: "light",
            fullscreen: false,
        }
        const configCache = localStorage.getItem("__NEXUS_CONFIG_v3.0__")
        if (configCache) {
            this.config = JSON.parse(configCache)
        } else {
            this.config = { ...this.defaultConfig }
        }
        this.html = document.documentElement
        this.sidebar = document.getElementById("layout-sidebar")

        window.config = this.config
    }

    updateTheme = () => {
        localStorage.setItem("__NEXUS_CONFIG_v3.0__", JSON.stringify(this.config))

        if (this.config.theme === "system") {
            this.html.removeAttribute("data-theme")
        } else {
            this.html.setAttribute("data-theme", this.config.theme)
        }

        if (this.sidebar) {
            if (
                this.config.sidebarTheme === "dark" &&
                ["light", "contrast"].includes(this.config.theme)
            ) {
                this.sidebar.setAttribute("data-theme", this.config.sidebarTheme)
            } else {
                this.sidebar.removeAttribute("data-theme")
            }
        }

        this.html.setAttribute("data-sidebar-theme", this.config.sidebarTheme)
        this.html.dir = this.config.direction

        if (this.config.fullscreen) {
            this.html.setAttribute("data-fullscreen", "")
        } else {
            this.html.removeAttribute("data-fullscreen")
        }

        if (this.config.fontFamily !== "default") {
            this.html.setAttribute("data-font-family", config.fontFamily)
        } else {
            this.html.removeAttribute("data-font-family")
        }
    }

    initEventListener = () => {
        const themeControls = document.querySelectorAll("[data-theme-control]")
        themeControls.forEach((control) => {
            control.addEventListener("click", () => {
                let theme = control.getAttribute("data-theme-control") ?? "light"
                if (theme === "toggle") {
                    theme = this.config.theme === "light" ? "dark" : "light"
                }
                this.config.theme = theme
                this.updateTheme()
            })
        })

        const sidebarThemeControls = document.querySelectorAll("[data-sidebar-theme-control]")
        sidebarThemeControls.forEach((control) => {
            control.addEventListener("click", () => {
                this.config.sidebarTheme =
                    control.getAttribute("data-sidebar-theme-control") ?? "light"
                this.updateTheme()
            })
        })

        const fontFamilyControls = document.querySelectorAll("[data-font-family-control]")
        fontFamilyControls.forEach((control) => {
            control.addEventListener("click", () => {
                this.config.fontFamily =
                    control.getAttribute("data-font-family-control") ?? "default"
                this.updateTheme()
            })
        })

        const dirControls = document.querySelectorAll("[data-dir-control]")
        dirControls.forEach((control) => {
            control.addEventListener("click", () => {
                this.config.direction = control.getAttribute("data-dir-control") ?? "ltr"
                this.updateTheme()
            })
        })

        const fullscreenControls = document.querySelectorAll("[data-fullscreen-control]")
        fullscreenControls.forEach((control) => {
            control.addEventListener("click", () => {
                if (document.fullscreenElement != null) {
                    this.config.fullscreen = false
                    document.exitFullscreen()
                } else {
                    this.config.fullscreen = true
                    this.html.requestFullscreen()
                }
                this.updateTheme()
            })
        })

        const resetControls = document.querySelectorAll("[data-reset-control]")
        resetControls.forEach((control) => {
            control.addEventListener("click", () => {
                this.config = { ...this.defaultConfig }
                if (document.fullscreenElement != null) {
                    document.exitFullscreen()
                }
                this.updateTheme()
            })
        })

        const fullscreenMedia = window.matchMedia("(display-mode: fullscreen)")
        const fullscreenListener = () => {
            this.config.fullscreen = fullscreenMedia.matches
            this.updateTheme()
        }
        fullscreenMedia.addEventListener("change", fullscreenListener)
        fullscreenListener()
    }

    initLeftmenu = () => {
        const initMenuActivation = () => {
            const menuItems = document.querySelectorAll(".sidebar-menu-activation  a")
            let currentURL = window.location.href
            if (window.location.pathname === "/") {
                currentURL += "dashboards-ecommerce.html"
            }
            menuItems.forEach((item) => {
                if (item.href === currentURL) {
                    item.classList.add("active")
                    const parentElement1 = item.parentElement.parentElement
                    if (parentElement1.classList.contains("collapse-content")) {
                        const inputElement1 = parentElement1.parentElement.querySelector("input")
                        if (inputElement1) {
                            inputElement1.checked = true
                        }
                    }
                }
            })
        }

        const scrollToActiveMenu = () => {
            const simplebarEl = document.querySelector("#layout-sidebar [data-simplebar]")
            const activatedItem = document.querySelector("#layout-sidebar .menu a.active")
            if (simplebarEl && activatedItem) {
                const simplebar = new SimpleBar(simplebarEl)
                const top = activatedItem?.getBoundingClientRect().top
                if (top && top !== 0) {
                    simplebar.getScrollElement().scrollTo({ top: top - 300, behavior: "smooth" })
                }
            }
        }

        initMenuActivation()
        scrollToActiveMenu()
    }

    afterInit = () => {
        this.initEventListener()
        this.initLeftmenu()
    }

    init = () => {
        this.updateTheme()
        window.addEventListener("DOMContentLoaded", this.afterInit)
    }
}

new LayoutCustomizer().init()
