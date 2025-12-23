const initModalOverviewChart = () => {
    const chartOptions = {
        chart: {
            height: 305,
            type: "line",
            background: "transparent",
            toolbar: { show: false },
        },
        stroke: {
            curve: "smooth",
            width: 2, // Keeping lines thin and sharp
        },
        dataLabels: { enabled: false },
        colors: ["#ff8b4b", "#4caf50", "#6c74f8"], // Three distinct colors
        legend: {
            show: true,
            horizontalAlign: "center",
            offsetY: 6,
        },
        series: [
            {
                name: "NeuraText",
                data: [
                    320, 330, 345, 310, 295, 340, 360, 345, 355, 370, 385, 400, 380, 360, 370, 390,
                    410, 420, 415,
                ],
            },
            {
                name: "VisioGen",
                data: [
                    300, 315, 295, 320, 335, 320, 290, 270, 300, 330, 340, 335, 290, 280, 310, 325,
                    310, 295, 285,
                ],
            },
            {
                name: "EchoWave",
                data: [
                    280, 295, 270, 260, 275, 300, 275, 280, 280, 300, 315, 305, 285, 290, 295, 310,
                    320, 310, 295,
                ],
            },
        ],
        xaxis: {
            categories: [
                "Feb 19",
                "Feb 20",
                "Feb 21",
                "Feb 22",
                "Feb 23",
                "Feb 24",
                "Feb 25",
                "Feb 26",
                "Feb 27",
                "Feb 28",
                "Feb 29",
                "Mar 1",
                "Mar 2",
                "Mar 3",
                "Mar 4",
                "Mar 5",
                "Mar 6",
                "Mar 7",
                "Mar 8",
            ],
            axisBorder: { show: false },
            axisTicks: { show: false },
        },
        yaxis: {
            labels: { show: true },
            axisBorder: { show: false },
            axisTicks: { show: false },
        },
        tooltip: {
            enabled: true,
            shared: true,
            intersect: false,
        },
        grid: { show: true, borderColor: "rgba(150,150,150,0.2)" },
        responsive: [
            {
                breakpoint: 450,
                options: {
                    stroke: { width: 1.5 },
                    xaxis: { tickAmount: 5 },
                },
            },
        ],
    }

    if (document.getElementById("modal-overview-chart")) {
        new ApexCharts(document.getElementById("modal-overview-chart"), chartOptions).render()
    }
}

const initResourceUtilizationChart = () => {
    const chartOptions = {
        chart: {
            type: "treemap",
            height: 275,
            background: "transparent",
            toolbar: { show: false },
        },
        plotOptions: {
            treemap: {
                distributed: true,
                enableShades: false,
                borderRadius: 8,
            },
        },
        dataLabels: {
            style: {
                colors: ["var(--color-base-content)"], // Blue text color
            },
        },
        stroke: {
            width: 4,
            colors: ["var(--color-base-100)"],
        },
        series: [
            {
                name: "AI Model Usage",
                data: [
                    { x: "NeuraText", y: 48 },
                    { x: "EchoWave", y: 44 },
                    { x: "DeepScribe", y: 42 },
                    { x: "SynthVoice", y: 40 },
                    { x: "VisioGen", y: 34 },
                    { x: "Cortex", y: 30 },
                    { x: "Crunch", y: 28 },
                ],
            },
        ],
        colors: [
            "#1b7efda0",
            "#fd731ea0",
            "#a153ffa0",
            "#28ff87a0",
            "#ff29aaa0",
            "#ffb01da0",
            "#A0B5C6a0",
        ],
        tooltip: {
            y: {
                formatter: (val) => `${val}% Usage`,
            },
        },
    }

    if (document.getElementById("resource-utilization-chart")) {
        new ApexCharts(document.getElementById("resource-utilization-chart"), chartOptions).render()
    }
}

document.addEventListener("DOMContentLoaded", () => {
    initModalOverviewChart()
    initResourceUtilizationChart()
})
