const initSimplePieChart = () => {
    const chartOptions = {
        chart: {
            height: 380,
            type: "pie",
            toolbar: {
                show: false,
            },
            background: "transparent",
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        title: {
            text: "Website Traffic",
            style: { fontWeight: "500" },
            align: "right",
        },
        tooltip: {
            enabled: true,
            y: {
                formatter: (value) => value + " Visitors",
            },
        },
        labels: ["Search", "Direct", "Referral", "Social", "Webinars", "Advertisement"],
        colors: ["#167bff", "#FDA403", "#FB6D48", "#A25772", "#8E7AB5", "#FFA299"],
        series: [428, 180, 88, 209, 91, 52],
    }

    if (document.getElementById("simple-pie-chart")) {
        new ApexCharts(document.getElementById("simple-pie-chart"), chartOptions).render()
    }
}

const initGradientDonutChart = () => {
    const seriesData = [50, 30, 40, 20, 25, 15, 10]
    const chartOptions = {
        chart: {
            type: "donut",
            height: 380,
            toolbar: {
                show: false,
            },
            background: "transparent",
        },
        title: {
            text: "Marketing Budget",
            style: { fontWeight: "500" },
            align: "right",
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        fill: {
            type: "gradient",
        },
        plotOptions: {
            pie: {
                startAngle: -45,
                endAngle: 315,
                donut: {
                    size: "60%",
                    labels: {
                        show: true,
                        value: {
                            formatter: (value) => `${value}K`,
                            color: "var(--color-base-content)",
                        },
                        total: {
                            show: true,
                            color: "#FF4560",
                            formatter: () => `${seriesData.reduce((acc, cur) => acc + cur, 0)}K`,
                        },
                    },
                },
            },
        },
        tooltip: {
            enabled: true,
            y: {
                formatter: (value) => `${value}K`,
            },
        },
        responsive: [
            {
                breakpoint: 480,
                options: {
                    chart: {
                        width: 200,
                    },
                    legend: {
                        position: "bottom",
                    },
                },
            },
        ],
        labels: [
            "Content",
            "Social Media",
            "SEO",
            "Paid Display",
            "Affiliate",
            "Magazine",
            "Promotional Items",
        ],
        colors: ["#167bff", "#FDA403", "#FB6D48", "#A25772", "#8E7AB5", "#FFA299", "#E3C878"],
        series: seriesData,
    }

    if (document.getElementById("gradient-donut-chart")) {
        new ApexCharts(document.getElementById("gradient-donut-chart"), chartOptions).render()
    }
}

const initPatternDonutChart = () => {
    const seriesData = [2512, 1003, 2009, 4322, 521]
    const chartOptions = {
        chart: {
            type: "donut",
            height: 380,
            toolbar: {
                show: false,
            },
            background: "transparent",
            dropShadow: {
                enabled: true,
                color: "#111",
                top: -1,
                left: 3,
                blur: 3,
                opacity: 0.2,
            },
        },
        title: {
            text: "Inventory",
            style: { fontWeight: "500" },
            align: "right",
            offsetX: -24,
        },
        legend: {
            position: "right",
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        fill: {
            type: "pattern",
            pattern: {
                style: ["squares", "verticalLines", "slantedLines", "circles", "horizontalLines"],
                width: 4,
                height: 4,
                strokeWidth: 1,
            },
        },
        plotOptions: {
            pie: {
                startAngle: -45,
                endAngle: 315,
                donut: {
                    size: "60%",
                    labels: {
                        show: true,
                        value: {
                            color: "var(--color-base-content)",
                            formatter: (value) => value + " Units",
                        },
                        total: {
                            show: true,
                            color: "#FF4560",
                            formatter: () =>
                                seriesData.reduce((acc, cur) => acc + cur, 0) + " Units",
                        },
                    },
                },
            },
        },
        tooltip: {
            enabled: true,
            y: {
                formatter: (value) => value + " Units",
            },
        },
        labels: ["Smartwatch", "Smartphone", "Tablet", "Headphone", "Laptop"],
        colors: ["#167bff", "#FB6D48", "#FDA403", "#A25772", "#8E7AB5"],
        series: seriesData,
    }

    if (document.getElementById("pattern-donut-chart")) {
        new ApexCharts(document.getElementById("pattern-donut-chart"), chartOptions).render()
    }
}

const initMonochromePieChart = () => {
    const chartOptions = {
        chart: {
            type: "pie",
            height: 380,
            toolbar: {
                show: false,
            },
            background: "transparent",
        },
        theme: {
            monochrome: {
                enabled: true,
                color: "#167bff",
                shadeTo: "light",
                shadeIntensity: 0.8,
            },
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        title: {
            text: "App Downloads",
            style: { fontWeight: "500" },
            align: "right",
        },
        tooltip: {
            enabled: true,
            y: {
                formatter: (value) => value + " Downloads",
            },
        },
        labels: ["Android", "iOS", "Windows", "MacOS", "Amazon FireOS"],
        series: [39243, 22187, 6947, 3375, 2688],
    }

    if (document.getElementById("monochrome-pie-chart")) {
        new ApexCharts(document.getElementById("monochrome-pie-chart"), chartOptions).render()
    }
}

document.addEventListener("DOMContentLoaded", function () {
    initSimplePieChart()
    initGradientDonutChart()
    initPatternDonutChart()
    initMonochromePieChart()
})
