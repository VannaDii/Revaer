const initChartStackedColumn = () => {
    const chartOptions = {
        xaxis: {
            categories: ["May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
            title: {
                text: "Monthly Cart Abandoned Count",
                style: { fontWeight: "500" },
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
        chart: {
            type: "bar",
            height: 380,
            toolbar: {
                show: true,
            },
            background: "transparent",
            stacked: true,
        },
        colors: ["#167bff", "#A25772", "#FB6D48", "#FDA403", "#8E7AB5"],
        fill: {
            type: "solid",
        },
        tooltip: {
            shared: true,
            intersect: false,
            inverseOrder: true,
        },
        plotOptions: {
            bar: {
                columnWidth: 40,
                borderRadius: 8,
                dataLabels: {
                    total: {
                        enabled: true,
                        offsetY: -8,
                        style: {
                            color: "#FFA299",
                        },
                    },
                },
            },
        },
        series: [
            {
                name: "Cart",
                data: [847, 723, 848, 573, 842, 973, 874, 942],
            },
            {
                name: "Checkout",
                data: [984, 697, 473, 784, 993, 824, 914, 973],
            },
            {
                name: "Shipping",
                data: [423, 673, 324, 473, 424, 347, 384, 442],
            },
            {
                name: "Payment",
                data: [384, 297, 362, 392, 427, 534, 377, 442],
            },
            {
                name: "Review",
                data: [642, 417, 304, 617, 439, 527, 689, 773],
            },
        ],
    }

    if (document.getElementById("stacked-column-chart")) {
        new ApexCharts(document.getElementById("stacked-column-chart"), chartOptions).render()
    }
}

const initChartDumbbellColumn = () => {
    const chartOptions = {
        xaxis: {
            tickPlacement: "on",
            title: {
                text: "Average Delivery Time (Days)",
                style: { fontWeight: "500" },
            },
        },
        yaxis: {
            min: 0,
            max: 10,
        },
        grid: {
            xaxis: {
                lines: {
                    show: true,
                },
            },
            yaxis: {
                lines: {
                    show: false,
                },
            },
        },
        chart: {
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
            type: "rangeBar",
        },
        fill: {
            type: "gradient",
            gradient: {
                type: "vertical",
                gradientToColors: ["#FB6D48"],
                inverseColors: true,
                stops: [0, 100],
            },
        },
        plotOptions: {
            bar: {
                columnWidth: 3,
                isDumbbell: true,
                dumbbellColors: [["#167bff", "#FB6D48"]],
            },
        },
        labels: ["Min Delivery Days", "Max Delivery Days"],
        legend: {
            show: true,
            showForSingleSeries: true,
            position: "bottom",
            horizontalAlign: "center",
            customLegendItems: ["Min Delivery Days", "Max Delivery Days"],
            markers: {
                fillColors: ["#167bff", "#FB6D48"],
            },
        },
        series: [
            {
                data: [
                    {
                        x: "California",
                        y: [2, 4],
                    },
                    {
                        x: "Nevada",
                        y: [2, 5],
                    },
                    {
                        x: "New York",
                        y: [1, 2],
                    },
                    {
                        x: "Arizona",
                        y: [1, 4],
                    },
                    {
                        x: "Vermont",
                        y: [2, 9],
                    },
                    {
                        x: "Texas",
                        y: [3, 6],
                    },
                    {
                        x: "Ohio",
                        y: [4, 7],
                    },
                    {
                        x: "Tennessee",
                        y: [2, 8],
                    },
                ],
            },
        ],
    }

    if (document.getElementById("dumbbell-column-chart")) {
        new ApexCharts(document.getElementById("dumbbell-column-chart"), chartOptions).render()
    }
}

const initChartRangeColumn = () => {
    const xAxisLabel = ["Aug", "Sep", "Oct", "Nov", "Dec"]
    const yAxisUsers = [
        [3, 5],
        [2, 6],
        [4, 6],
        [3, 7],
        [2, 7],
    ]
    const yAxisPremiumSubscriber = [
        [2, 3],
        [2, 4],
        [2, 4],
        [1, 5],
        [1, 3],
    ]

    const chartOptions = {
        xaxis: {
            title: {
                text: "Customer Churn Rate (%)",
                style: { fontWeight: "500" },
            },
        },
        yaxis: {
            min: 0,
        },
        tooltip: {
            y: {
                formatter: (val) => val + "%",
            },
        },
        legend: {
            position: "top",
        },
        grid: {
            show: false,
        },
        stroke: {
            show: true,
            width: 1,
            colors: ["var(--color-base-100)"],
        },
        chart: {
            height: 380,
            toolbar: {
                show: true,
            },
            type: "rangeBar",
            background: "transparent",
        },
        colors: ["#167bff", "#FDA403"],
        fill: {
            type: "solid",
        },
        plotOptions: {
            bar: {
                columnWidth: 40,
            },
        },
        dataLabels: {
            enabled: true,
            formatter: (val, opts) => {
                const dataValue = opts.w.config.series[opts.seriesIndex].data[opts.dataPointIndex].y
                return dataValue[1] - dataValue[0] + "%"
            },
        },
        series: [
            {
                name: "User",
                data: xAxisLabel.map((label, index) => ({
                    x: label,
                    y: yAxisUsers[index],
                })),
            },
            {
                name: "Premium Subscriber",
                data: xAxisLabel.map((label, index) => ({
                    x: label,
                    y: yAxisPremiumSubscriber[index],
                })),
            },
        ],
    }

    if (document.getElementById("range-column-chart")) {
        new ApexCharts(document.getElementById("range-column-chart"), chartOptions).render()
    }
}

const initNegativeValueColumn = () => {
    const chartOptions = {
        series: [
            {
                name: "Cash Flow",
                data: [
                    1.45, 5.42, 5.9, -0.42, -12.6, -18.1, -18.2, -14.16, -11.1, -6.09, 0.34, 3.88,
                    13.07, 5.8, 2, 7.37, 8.1, 13.57, 15.75, 17.1, 19.8, -27.03, -54.4, -47.2, -43.3,
                    -18.6, -48.6, -41.1, -39.6, -37.6, -29.4, -21.4, -2.4,
                ],
            },
        ],
        chart: {
            type: "bar",
            height: 350,
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
        },
        plotOptions: {
            bar: {
                colors: {
                    ranges: [
                        {
                            from: -1000,
                            to: -46,
                            color: "#FDA403",
                        },
                        {
                            from: -20,
                            to: 5,
                            color: "#A25772",
                        },
                        {
                            from: 5,
                            to: 1000,
                            color: "#167bff",
                        },
                    ],
                },
                columnWidth: "80%",
            },
        },
        dataLabels: {
            enabled: false,
        },
        yaxis: {
            title: {
                text: "Growth",
            },
            labels: {
                formatter: function (y) {
                    return y.toFixed(0) + "%"
                },
            },
        },
        xaxis: {
            type: "datetime",
            categories: [
                "2011-01-01",
                "2011-02-01",
                "2011-03-01",
                "2011-04-01",
                "2011-05-01",
                "2011-06-01",
                "2011-07-01",
                "2011-08-01",
                "2011-09-01",
                "2011-10-01",
                "2011-11-01",
                "2011-12-01",
                "2012-01-01",
                "2012-02-01",
                "2012-03-01",
                "2012-04-01",
                "2012-05-01",
                "2012-06-01",
                "2012-07-01",
                "2012-08-01",
                "2012-09-01",
                "2012-10-01",
                "2012-11-01",
                "2012-12-01",
                "2013-01-01",
                "2013-02-01",
                "2013-03-01",
                "2013-04-01",
                "2013-05-01",
                "2013-06-01",
                "2013-07-01",
                "2013-08-01",
                "2013-09-01",
            ],
            labels: {
                rotate: -90,
            },
        },
    }

    if (document.getElementById("negative-value-column-chart")) {
        new ApexCharts(
            document.getElementById("negative-value-column-chart"),
            chartOptions
        ).render()
    }
}

document.addEventListener("DOMContentLoaded", function () {
    initChartStackedColumn()
    initChartDumbbellColumn()
    initChartRangeColumn()
    initNegativeValueColumn()
})
