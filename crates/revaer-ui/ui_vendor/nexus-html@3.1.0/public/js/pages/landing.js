const initTopbar = () => {
    const topbar = document.getElementById("landing-topbar")
    if (topbar) {
        let scrollPosition = 0,
            scrolling,
            prevScrollPosition = 0

        const onChangeScroll = () => {
            topbar.setAttribute("data-at-top", scrollPosition < 30 ? "true" : "false")
            if (scrollPosition < 500) {
                topbar.removeAttribute("data-scrolling")
                scrolling = undefined
            } else {
                if (scrollPosition - prevScrollPosition > 0) {
                    topbar.setAttribute("data-scrolling", "down")
                } else if (scrollPosition - prevScrollPosition < 0) {
                    topbar.setAttribute("data-scrolling", "up")
                }
            }
        }

        const handleScroll = () => {
            setTimeout(() => {
                prevScrollPosition = scrollPosition
                scrollPosition = window.scrollY
                onChangeScroll()
            }, 200)
        }

        window.addEventListener("scroll", handleScroll, { passive: true })
        handleScroll()
    }
}

const initHeroSwiper = () => {
    if (document.querySelector("#hero-swiper")) {
        new Swiper("#hero-swiper", {
            slidesPerView: 1,
            loop: true,
            speed: 1500,
            autoplay: {
                delay: 5000,
            },
            spaceBetween: 20,
            navigation: {
                prevEl: ".hero-swiper-button-prev",
                nextEl: ".hero-swiper-button-next",
            },
        })
    }
}

const initTestimonialSwiper = () => {
    if (document.querySelector("#testimonial-swiper")) {
        new Swiper("#testimonial-swiper", {
            slidesPerView: 1,
            loop: true,
            speed: 1500,
            autoplay: {
                delay: 5000,
            },
            spaceBetween: 20,
            navigation: {
                prevEl: ".testimonial-swiper-button-prev",
                nextEl: ".testimonial-swiper-button-next",
            },
        })
    }
}
document.addEventListener("DOMContentLoaded", () => {
    initTopbar()
    initHeroSwiper()
    initTestimonialSwiper()
})
