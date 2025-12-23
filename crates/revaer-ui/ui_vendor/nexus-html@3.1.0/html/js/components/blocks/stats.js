const baseChartOptions = {
    chart: {
        height: 28,
        type: "line",
        background: "transparent",
        toolbar: { show: false },
        sparkline: {
            enabled: true,
        },
    },
    stroke: {
        curve: "smooth",
        width: 2,
    },
    dataLabels: { enabled: false },
    legend: {
        show: false,
    },
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
        ],
        axisBorder: {
            show: false,
        },
        labels: {
            show: false,
        },
        axisTicks: {
            show: false,
        },
    },
    yaxis: {
        labels: { show: false },
        axisBorder: { show: false },
        show: false,
        axisTicks: { show: false },
    },
    tooltip: {
        enabled: true,
        shared: true,
        intersect: false,
    },
    grid: { show: false },
}

const renderChart = (chartId, title, data, color) => {
    if (document.getElementById(chartId)) {
        new ApexCharts(document.getElementById(chartId), {
            ...baseChartOptions,
            colors: [color],
            yaxis: {
                ...baseChartOptions.yaxis,
                min: Math.min(...data) * 0.85,
                max: Math.max(...data) * 1.15,
            },
            series: [
                {
                    name: title,
                    data,
                },
            ],
        }).render()
    }
}

renderChart(
    "dashboard-stats-demo-1",
    "Total Sales",
    [120, 150, 180, 160, 200, 230, 210, 250],
    "#4caf50"
)
renderChart(
    "dashboard-stats-demo-2",
    "New Customers",
    [80, 90, 100, 95, 110, 130, 125, 140],
    "#6c74f8"
)
renderChart(
    "dashboard-stats-demo-3",
    "Churn Rate",
    [3.2, 3.1, 3.0, 2.9, 3.1, 3.0, 2.8, 2.8],
    "#f44336"
)
renderChart("dashboard-stats-demo-4", "Revenue Growth", [15, 18, 20, 22, 21, 23, 24, 25], "#ff8b4b")
