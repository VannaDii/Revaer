const initChartLabelLine = () => {
    const chartOptions = {
        chart: {
            type: "line",
            height: 380,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
        },
        xaxis: {
            categories: ["May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
            title: {
                text: "Monthly Revenue by Platform",
                style: { fontWeight: "500" },
            },
        },
        yaxis: {
            labels: {
                formatter: (value) => (value / 100).toFixed(0) + "K",
                offsetX: -5,
            },
            min: 3000,
        },
        stroke: {
            curve: "smooth",
            width: 2,
        },
        dataLabels: {
            enabled: true,
            formatter: (value) => (Number(value) / 100).toFixed(0),
            background: {
                borderColor: "var(--color-base-100)",
            },
        },
        tooltip: {
            y: {
                formatter: (value) => `$${(Number(value) / 100).toFixed(2)}K`,
            },
        },
        colors: ["#167bff", "#A25772", "#FB6D48", "#FDA403"],
        series: [
            {
                name: "eBay",
                data: [12105, 11562, 10697, 12126, 12817, 12070, 12403, 12758],
            },
            {
                name: "Walmart",
                data: [8866, 9566, 8821, 8799, 9272, 9109, 9272, 8601],
            },
            {
                name: "Amazon",
                data: [7680, 7685, 7293, 6952, 6568, 7572, 6538, 6498],
            },
            {
                name: "Best Buy",
                data: [4537, 5892, 4271, 4923, 5186, 4419, 5548, 4720],
            },
        ],
    }

    if (document.getElementById("label-line-chart")) {
        new ApexCharts(document.getElementById("label-line-chart"), chartOptions).render()
    }
}

const initChartStepLine = () => {
    const chartOptions = {
        chart: {
            type: "line",
            height: 380,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
        },
        xaxis: {
            categories: [
                "Jul-2",
                "Aug-1",
                "Aug-2",
                "Sep-1",
                "Sep-2",
                "Oct-1",
                "Oct-2",
                "Nov-1",
                "Nov-2",
                "Dec-1",
            ],
            title: {
                text: "Customer Support Ticket Volume",
                style: { fontWeight: "500" },
            },
        },
        tooltip: {
            y: {
                formatter: (val) => val + " Tickets",
            },
        },
        stroke: {
            curve: "stepline",
            width: 2,
        },
        colors: ["#167bff"],
        series: [
            {
                name: "Volume",
                data: [144, 154, 121, 112, 143, 233, 223, 166, 166, 158],
            },
        ],
    }

    if (document.getElementById("step-line-chart")) {
        new ApexCharts(document.getElementById("step-line-chart"), chartOptions).render()
    }
}

const initChartSyncingLine = () => {
    const xAxisOption = {
        type: "datetime",
        categories: [
            1735287404201, 1735201004201, 1735114604201, 1735028204201, 1734941804201,
            1734855404201, 1734769004201, 1734682604201, 1734596204201, 1734509804201,
            1734423404201, 1734337004201, 1734250604201, 1734164204201, 1734077804201,
        ],
        max: 1735287404201,
    }

    const orderChartOptions = {
        chart: {
            type: "line",
            height: 120,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
            id: "order-placed",
            group: "order",
        },
        xaxis: xAxisOption,

        stroke: {
            width: 2,
        },
        series: [
            {
                color: "#167bff",
                name: "Orders",
                data: [112, 108, 137, 172, 184, 190, 198, 192, 145, 130, 121, 145, 134, 128, 80],
            },
        ],
    }

    const revenueChartOptions = {
        chart: {
            type: "line",
            height: 120,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
            id: "order-revenue",
            group: "order",
        },
        xaxis: xAxisOption,
        yaxis: {
            labels: {
                formatter: (val) => (val / 1000).toFixed(0) + "K",
            },
        },
        tooltip: {
            y: {
                formatter: (val) => `$${(val / 1000).toFixed(2)}K`,
            },
        },
        stroke: {
            width: 2,
        },
        series: [
            {
                color: "#FFC700",
                name: "Revenue",
                data: [
                    11326.56, 12121.92, 21411.73, 28822.04, 18217.84, 18331.2, 18117, 18958.08,
                    14685.6, 19505.2, 19390.25, 24960.3, 19194.16, 18012.16, 7988,
                ],
            },
        ],
    }

    const averageChartOptions = {
        chart: {
            type: "line",
            height: 120,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
            id: "order-average",
            group: "order",
        },
        xaxis: xAxisOption,
        yaxis: {
            labels: {
                formatter: (val) => val.toFixed(0),
            },
        },
        tooltip: {
            y: {
                formatter: (val) => `$${val}`,
            },
        },
        stroke: {
            width: 2,
        },
        series: [
            {
                color: "#FFAD84",
                name: "Average",
                data: [
                    101.13, 112.24, 156.29, 167.57, 99.01, 96.48, 91.5, 98.74, 101.28, 150.04,
                    160.25, 172.14, 143.24, 140.72, 99.85,
                ],
            },
        ],
    }

    if (document.getElementById("syncing-order-line-chart")) {
        new ApexCharts(
            document.getElementById("syncing-order-line-chart"),
            orderChartOptions
        ).render()
    }

    if (document.getElementById("syncing-revenue-line-chart")) {
        new ApexCharts(
            document.getElementById("syncing-revenue-line-chart"),
            revenueChartOptions
        ).render()
    }
    if (document.getElementById("syncing-average-line-chart")) {
        new ApexCharts(
            document.getElementById("syncing-average-line-chart"),
            averageChartOptions
        ).render()
    }
}

const initChartAnnotationLine = () => {
    const xAxisLabels = [
        "Sep 21-30",
        "Oct 1-10",
        "Oct 11-20",
        "Oct 21-31",
        "Nov 1-10",
        "Nov 11-20",
        "Nov 21-30",
        "Dec 1-10",
    ]

    const chartOptions = {
        chart: {
            type: "line",
            height: 380,
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
            },
            background: "transparent",
        },
        xaxis: {
            categories: xAxisLabels,
            title: {
                text: "Sales",
                style: { fontWeight: "500" },
            },
        },
        yaxis: {
            labels: {
                formatter: (val) => val.toFixed(0) + "K",
            },
        },
        tooltip: {
            y: {
                formatter: (val) => `$${val}K`,
            },
        },
        annotations: {
            yaxis: [
                {
                    y: 202,
                    strokeDashArray: 0,
                    borderColor: "#FDA403",
                    label: {
                        style: {
                            color: "#fff",
                            background: "#FDA403",
                        },
                        text: "Target",
                        borderWidth: 0,
                    },
                },
            ],
            xaxis: [
                {
                    x: xAxisLabels[2],
                    strokeDashArray: 0,
                    borderColor: "#A25772",
                    label: {
                        style: {
                            color: "#fff",
                            background: "#A25772",
                        },
                        text: "Start Of Sale",
                        borderWidth: 0,
                    },
                },
                {
                    x: xAxisLabels[4],
                    x2: xAxisLabels[5],
                    strokeDashArray: 0,
                    borderColor: "#8E7AB5",
                    label: {
                        style: {
                            color: "#fff",
                            background: "#8E7AB5",
                        },
                        text: "Festive Season",
                        borderWidth: 0,
                    },
                },
            ],
            points: [
                {
                    x: xAxisLabels[6],
                    y: 196.78,
                    marker: {
                        size: 6,
                        fillColor: "#FF4560",
                        strokeColor: "FF4560",
                    },
                    label: {
                        borderColor: "#FF4560",
                        offsetY: 36,
                        style: {
                            color: "#fff",
                            background: "#FF4560",
                        },
                        borderWidth: 0,
                        text: "Production Down",
                    },
                },
            ],
        },
        stroke: {
            curve: "smooth",
            width: 2,
        },
        colors: ["#167bff"],
        series: [
            {
                name: "Sales",
                data: [114.87, 105.88, 90.58, 135.43, 86.39, 212.99, 196.78, 143.76],
            },
        ],
    }

    if (document.getElementById("annotation-line-chart")) {
        new ApexCharts(document.getElementById("annotation-line-chart"), chartOptions).render()
    }
}

document.addEventListener("DOMContentLoaded", function () {
    initChartLabelLine()
    initChartStepLine()
    initChartSyncingLine()
    initChartAnnotationLine()
})
