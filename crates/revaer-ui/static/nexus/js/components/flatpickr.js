window.addEventListener("DOMContentLoaded", () => {
    flatpickr("#date-flatpickr-demo", {
        defaultDate: Date.now(),
    })

    flatpickr("#disabled-flatpickr-demo", {
        defaultDate: Date.now(),
    })

    flatpickr("#time-flatpickr-demo", {
        defaultDate: new Date(),
        mode: "time",
    })

    flatpickr("#time-24-flatpickr-demo", {
        defaultDate: new Date(),
        mode: "time",
        time_24hr: true,
    })

    flatpickr("#date-time-flatpickr-demo", {
        defaultDate: new Date(),
        enableTime: true,
    })

    flatpickr("#human-friendly-flatpickr-demo", {
        defaultDate: Date.now(),
        altInput: true,
        altFormat: "F j, Y",
        dateFormat: "Y-m-d",
    })

    flatpickr("#controls-flatpickr-demo", {
        defaultDate: Date.now(),
        wrap: true,
    })

    flatpickr("#inline-flatpickr-demo", {
        defaultDate: Date.now(),
        inline: true,
    })

    flatpickr("#min-max-flatpickr-demo", {
        defaultDate: new Date(),
        inline: true,
        minDate: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000),
        maxDate: new Date(Date.now() + 2 * 24 * 60 * 60 * 1000),
    })

    flatpickr("#enabled-dates-flatpickr-demo", {
        defaultDate: new Date(),
        inline: true,
        enable: [
            new Date(Date.now() - 24 * 60 * 60 * 1000),
            new Date(),
            new Date(Date.now() + 24 * 60 * 60 * 1000),
        ],
    })

    flatpickr("#disabled-dates-flatpickr-demo", {
        defaultDate: new Date(Date.now() + 2 * 24 * 60 * 60 * 1000),
        inline: true,
        disable: [
            new Date(Date.now() - 24 * 60 * 60 * 1000),
            Date.now(),
            new Date(Date.now() + 24 * 60 * 60 * 1000),
        ],
    })

    flatpickr("#multiple-flatpickr-demo", {
        defaultDate: [new Date(), new Date(Date.now() + 2 * 24 * 60 * 60 * 1000)],
        inline: true,
        mode: "multiple",
    })

    flatpickr("#range-flatpickr-demo", {
        defaultDate: [new Date(), new Date(Date.now() + 4 * 24 * 60 * 60 * 1000)],
        inline: true,
        mode: "range",
    })

    flatpickr("#week-number-flatpickr-demo", {
        defaultDate: Date.now(),
        inline: true,
        weekNumbers: true,
    })

    flatpickr("#range-plugin-flatpickr-demo-1", {
        mode: "range",
        plugins: [new rangePlugin({ input: "#range-plugin-flatpickr-demo-2" })],
    })

    flatpickr("#confirm-plugin-flatpickr-demo", {
        defaultDate: Date.now(),
        closeOnSelect: false,
        plugins: [
            confirmDatePlugin({
                confirmIcon: "<span class='iconify lucide--check'></span>",
                confirmText: "OK",
                showAlways: true,
            }),
        ],
    })

    flatpickr("#week-plugin-flatpickr-demo", {
        defaultDate: Date.now(),
        plugins: [weekSelect()],
    })

    flatpickr("#month-plugin-flatpickr-demo", {
        defaultDate: Date.now(),
        plugins: [monthSelectPlugin()],
    })

    flatpickr("#hindi-locale-flatpickr-demo", {
        inline: true,
        locale: "hi",
        altInput: true,
        altFormat: "F j, Y",
        dateFormat: "Y-m-d",
        defaultDate: Date.now(),
    })

    flatpickr("#mandarin-flatpickr-demo", {
        inline: true,
        locale: "zh",
        altInput: true,
        altFormat: "F j, Y",
        dateFormat: "Y-m-d",
        defaultDate: Date.now(),
    })
})
