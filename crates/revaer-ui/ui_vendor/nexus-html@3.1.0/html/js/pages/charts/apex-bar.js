const initMarkerChart = () => {
    const data = [
        {
            y: 488,
            goals: {
                name: "Predicted Sales",
                value: 680,
                strokeDashArray: 2,
            },
        },
        {
            y: 680,
            goals: {
                name: "Predicted Sales",
                value: 710,
                strokeDashArray: 2,
            },
        },
        {
            y: 722,
            goals: {
                name: "Predicted Sales",
                value: 680,
            },
        },
        {
            y: 539,
            goals: {
                name: "Predicted Sales",
                value: 594,
                strokeDashArray: 2,
            },
        },
        {
            y: 461,
            goals: {
                name: "Predicted Sales",
                value: 397,
            },
        },
        {
            y: 322,
            goals: {
                name: "Predicted Sales",
                value: 300,
            },
        },
    ]
    const currentYear = new Date().getFullYear()
    const seriesWithGoals = data.map((data, index) => ({
        ...data,
        x: (currentYear - index).toString(),
        goals: [{ ...data.goals, strokeWidth: 6, strokeColor: "#EB6440" }],
    }))

    const chartOptions = {
        xaxis: {
            type: "numeric",
            title: { text: "(Million USD)", style: { fontWeight: "500" } },
            labels: {
                formatter: (value) => value + "M",
            },
        },
        tooltip: {
            y: {
                formatter: (value) => value + "M",
            },
        },
        grid: {
            show: false,
        },
        chart: {
            type: "bar",
            height: 380,
            toolbar: {
                show: true,
            },
            background: "transparent",
        },
        colors: ["#167bff"],
        fill: {
            type: "solid",
        },
        legend: {
            show: true,
            showForSingleSeries: true,
        },
        plotOptions: {
            bar: {
                horizontal: true,
                barHeight: 28,
            },
        },
        series: [
            {
                name: "Total Sales",
                data: seriesWithGoals,
            },
        ],
    }

    if (document.getElementById("marker-with-bar-chart")) {
        new ApexCharts(document.getElementById("marker-with-bar-chart"), chartOptions).render()
    }
}

const initGroupedChart = () => {
    const chartOptions = {
        xaxis: {
            categories: ["Atlas", "Phoenix", "Zenith", "Forge"],
            title: {
                text: "Expense Breakdown",
                style: { fontWeight: "500" },
            },
            labels: {
                formatter: (value) => value + "K",
            },
        },
        grid: {
            show: false,
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        tooltip: {
            shared: true,
            intersect: false,
            y: {
                formatter: (value) => value + "K",
            },
        },
        chart: {
            type: "bar",
            height: 380,
            toolbar: {
                show: true,
            },
            background: "transparent",
        },
        colors: ["#167bff", "#FDA403", "#FB6D48", "#8E7AB5"],
        fill: {
            type: "solid",
        },
        plotOptions: {
            bar: {
                horizontal: true,
                barHeight: 14,
            },
        },
        series: [
            {
                name: "Labor",
                data: [122, 215, 180, 210],
            },
            {
                name: "Material",
                data: [158, 169, 143, 133],
            },
            {
                name: "Marketing",
                data: [146, 98, 123, 111],
            },
            {
                name: "Travel",
                data: [59, 42, 71, 28],
            },
        ],
    }

    if (document.getElementById("grouped-bar-chart")) {
        new ApexCharts(document.getElementById("grouped-bar-chart"), chartOptions).render()
    }
}

const initStackedChart = () => {
    const chartOptions = {
        xaxis: {
            categories: ["Alabama", "Florida", "Georgia", "Nevada", "Texas", "South Carolina"],
            title: {
                text: "Regional Ratio Of Sale",
                style: { fontWeight: "500" },
            },
            labels: {
                formatter: (value) => value + "%",
            },
        },
        grid: {
            show: false,
        },
        yaxis: {},
        tooltip: {
            shared: true,
            intersect: false,
            y: {
                formatter: (value) => value + "%",
            },
        },
        chart: {
            type: "bar",
            height: 380,
            toolbar: {
                show: true,
            },
            background: "transparent",
            stacked: true,
            stackType: "100%",
        },
        colors: ["#167bff", "#FB6D48", "#A25772", "#FDA403", "#8E7AB5"],
        fill: {
            type: "solid",
        },
        plotOptions: {
            bar: {
                horizontal: true,
                barHeight: 28,
            },
        },
        series: [
            {
                name: "Clothing",
                data: [12, 34, 20, 27, 22, 14],
            },
            {
                name: "Electronics",
                data: [24, 20, 12, 18, 23, 8],
            },
            {
                name: "Homeware",
                data: [10, 18, 9, 10, 21, 29],
            },
            {
                name: "Cosmetics",
                data: [28, 18, 39, 40, 25, 30],
            },
            {
                name: "Toys",
                data: [26, 10, 20, 5, 9, 19],
            },
        ],
    }

    if (document.getElementById("stacked-bar-chart")) {
        new ApexCharts(document.getElementById("stacked-bar-chart"), chartOptions).render()
    }
}

const initNegativeValueChart = () => {
    const chartOptions = {
        xaxis: {
            categories: [
                "Smart Watches",
                "Wireless Headphones",
                "Earbuds",
                "Wired Earphones",
                "Speakers",
                "Soundbars",
                "Personalised Products",
                "Accessories",
            ],
            title: {
                text: "Product Review Sentiment",
                style: { fontWeight: "500" },
            },
            labels: {
                formatter: (value) => Math.abs(Number(value)).toString(),
            },
        },
        dataLabels: {
            formatter: (value) => Math.abs(Number(value)).toString(),
        },
        tooltip: {
            shared: false,
            y: {
                formatter: (val) => Math.abs(val).toString() + " Reviews",
            },
        },
        grid: {
            show: false,
        },
        chart: {
            type: "bar",
            height: 380,
            toolbar: {
                show: true,
            },
            background: "transparent",
            stacked: true,
        },
        colors: ["#167bff", "#FDA403"],
        fill: {
            type: "solid",
        },
        plotOptions: {
            bar: {
                horizontal: true,
                barHeight: 28,
            },
        },
        series: [
            {
                name: "Positive",
                data: [379, 293, 411, 387, 242, 434, 321, 357],
            },
            {
                name: "Negative",
                data: [151, 208, 90, 113, 268, 76, 189, 88].map((value) => -value),
            },
        ],
    }

    if (document.getElementById("negative-value-bar-chart")) {
        new ApexCharts(document.getElementById("negative-value-bar-chart"), chartOptions).render()
    }
}

document.addEventListener("DOMContentLoaded", function () {
    initMarkerChart()
    initGroupedChart()
    initStackedChart()
    initNegativeValueChart()
})
