const initCustomerAcquisitionChart = () => {
    const chartOptions = {
        chart: {
            height: 356,
            sparkline: {
                enabled: false,
            },
            toolbar: {
                show: false,
            },
            zoom: {
                enabled: false,
            },
            background: "transparent",
        },
        forecastDataPoints: {
            count: 2,
            dashArray: [6, 4],
        },
        grid: {
            show: false,
        },
        yaxis: {
            show: false,
            min: 125,
            max: 181,
        },
        xaxis: {
            categories: Array.from({ length: 15 }, (_, index) => index + 1),
        },
        tooltip: {
            y: {
                formatter: (val) => val.toString(),
            },
        },
        stroke: {
            curve: "stepline",
            width: [2, 1.5],
        },
        colors: ["#167bff", "rgba(150,150,150,0.3)"],
        series: [
            {
                name: "Customer",
                data: [144, 150, 146, 154, 150, 155, 160, 155, 140, 155, 160, 180, 170, 165, 165],
            },
            {
                name: "Advertise",
                data: [140, 142, 142, 140, 146, 148, 150, 136, 130, 133, 145, 148, 158, 150, 150],
            },
        ],
    }

    if (document.getElementById("customer-acquisition-chart")) {
        new ApexCharts(document.getElementById("customer-acquisition-chart"), chartOptions).render()
    }
}

const initRevenueStatisticsChart = () => {
    const chartOptions = {
        chart: {
            height: 288,
            type: "bar",
            stacked: true,
            background: "transparent",
            toolbar: {
                show: false,
            },
        },

        plotOptions: {
            bar: {
                borderRadius: 8,
                borderRadiusApplication: "end",
                borderRadiusWhenStacked: "last",
                colors: {
                    backgroundBarColors: ["rgba(150,150,150,0.07)"],
                    backgroundBarRadius: 8,
                },
                columnWidth: "45%",
                barHeight: "100%",
            },
        },
        dataLabels: {
            enabled: false,
        },
        colors: ["#ff8b4b", "#6c74f8"],
        legend: {
            show: true,
            horizontalAlign: "center",
            offsetX: 0,
            offsetY: 6,
        },
        series: [
            {
                name: "Orders",
                data: [10, 12, 14, 16, 18, 20, 14, 16, 24, 12],
            },
            {
                name: "Revenue",
                data: [15, 24, 21, 28, 30, 40, 22, 32, 48, 20],
            },
        ],
        xaxis: {
            categories: [
                new Date("1/1/2016"),
                new Date("1/1/2017"),
                new Date("1/1/2018"),
                new Date("1/1/2019"),
                new Date("1/1/2020"),
                new Date("1/1/2021"),
                new Date("1/1/2022"),
                new Date("1/1/2023"),
                new Date("1/1/2024"),
                new Date("1/1/2025"),
            ],
            axisBorder: {
                show: false,
            },
            axisTicks: {
                show: false,
            },
            labels: {
                formatter: (val) => {
                    return new Date(val).getFullYear().toString()
                },
            },
        },
        yaxis: {
            axisBorder: {
                show: false,
            },
            axisTicks: {
                show: false,
            },
            labels: {
                show: false,
            },
        },

        tooltip: {
            enabled: true,
            shared: true,
            intersect: false,
        },
        grid: {
            show: false,
        },
        responsive: [
            {
                breakpoint: 450,
                options: {
                    plotOptions: {
                        bar: {
                            borderRadius: 4,
                        },
                    },
                    xaxis: {
                        tickAmount: 3,
                    },
                },
            },
        ],
    }

    if (document.getElementById("revenue-statics-chart")) {
        new ApexCharts(document.getElementById("revenue-statics-chart"), chartOptions).render()
    }
}

const initGlobalSalesChart = () => {
    const data = [
        {
            name: "Turkey",
            orders: 9,
        },
        {
            name: "India",
            orders: 12,
        },
        {
            name: "Canada",
            orders: 13,
        },
        {
            name: "US",
            orders: 16,
        },
        {
            name: "Netherlands",
            orders: 14,
        },
        {
            name: "Italy",
            orders: 17,
        },
        {
            name: "Other",
            orders: 19,
        },
    ]

    const chartOptions = {
        chart: {
            height: 344,
            type: "bar",
            parentHeightOffset: 0,
            background: "transparent",
            toolbar: {
                show: false,
            },
        },
        plotOptions: {
            bar: {
                horizontal: true,
                borderRadius: 4,
                distributed: true,
                borderRadiusApplication: "end",
            },
        },
        dataLabels: {
            enabled: true,
            textAnchor: "start",
            style: {
                colors: ["#fff"],
            },
            formatter: function (val, opt) {
                return opt.w.globals.labels[opt.dataPointIndex] + ":  " + val
            },
            offsetX: -10,
            dropShadow: {
                enabled: false,
            },
        },
        series: [
            {
                data: data.map((country) => country.orders),
            },
        ],
        legend: {
            show: false,
        },
        stroke: {
            width: 0,
            colors: ["#fff"],
        },
        xaxis: {
            categories: data.map((country) => country.name),
        },
        yaxis: {
            labels: {
                show: false,
            },
        },
        grid: {
            show: false,
        },

        tooltip: {
            theme: "dark",
            x: {
                show: false,
            },
            y: {
                formatter: (val) => `${val}%`,
            },
        },
        colors: ["#7179ff", "#4bcd89", "#ff6c88", "#5cb7ff", "#9071ff", "#ff5892", "#ff8b4b"],
    }

    if (document.getElementById("global-sales-chart")) {
        new ApexCharts(document.getElementById("global-sales-chart"), chartOptions).render()
    }
}

document.addEventListener("DOMContentLoaded", () => {
    initCustomerAcquisitionChart()
    initRevenueStatisticsChart()
    initGlobalSalesChart()
})
