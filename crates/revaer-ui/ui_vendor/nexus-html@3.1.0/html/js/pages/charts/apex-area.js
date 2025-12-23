window.ApexCharts = ApexCharts

const initSplineChart = () => {
    const chartOptions = {
        xaxis: {
            categories: ["Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        },
        yaxis: {
            labels: {
                formatter: function (value) {
                    return `$${value}K`
                },
            },
        },
        chart: {
            type: "area",
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
        colors: ["#167bff", "#FDA403"],
        fill: {
            type: "solid",
            opacity: 0.6,
        },
        stroke: {
            curve: "smooth",
            width: 2,
        },
        dataLabels: {
            enabled: false,
        },
        legend: {
            show: true,
            position: "top",
        },
        series: [
            {
                name: "Basic Plan",
                data: [31, 40, 28, 51, 42, 72, 60],
            },
            {
                name: "Premium Plan",
                data: [11, 32, 45, 32, 34, 52, 41],
            },
        ],
    }

    if (document.getElementById("spline-area-chart")) {
        new ApexCharts(document.getElementById("spline-area-chart"), chartOptions).render()
    }
}

const initNegativeValueChart = () => {
    const negativeDataValues = {
        north: [319, 320, 324, 344, 345, 340, 329, 315, 325, 328],
        northeast: [227, 254, 223, 233, 262, 254, 249, 267, 302, 209],
        midwest: [147, 155, 123, 127, 157, 157, 133, 169, 199, 121],
        west: [168, 91, 48, 20, -1, -37, -88, -130, -90, -78],
    }

    const chartOptions = {
        xaxis: {
            categories: Array.from(
                { length: 10 },
                (_, index) => new Date().getFullYear() - index - 1
            ).reverse(),
        },
        yaxis: {
            labels: {
                formatter: function (value) {
                    return `${value < 0 ? "-" : ""}$${Math.abs(value)}K`
                },
            },
        },
        chart: {
            type: "area",
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
        colors: ["#167bff", "#FFC470", "#67C6E3", "#FFC700"],
        fill: {
            type: "solid",
            opacity: 0.6,
        },
        stroke: {
            curve: "smooth",
            width: 1,
        },
        dataLabels: {
            enabled: false,
        },
        legend: {
            show: true,
            position: "top",
        },
        series: [
            {
                name: "North",
                data: negativeDataValues.north,
            },
            {
                name: "Northeast",
                data: negativeDataValues.northeast,
            },
            {
                name: "Midwest",
                data: negativeDataValues.midwest,
            },
            {
                name: "West",
                data: negativeDataValues.west,
            },
        ],
    }

    if (document.getElementById("negative-value-area-chart")) {
        new ApexCharts(document.getElementById("negative-value-area-chart"), chartOptions).render()
    }
}

const initIrregularTimeSeriesChart = () => {
    const dataA = [
        {
            x: new Date("1/1/2025"),
            y: 150,
        },
        {
            x: new Date("1/2/2025"),
            y: 160,
        },
        {
            x: new Date("1/3/2025"),
            y: 145,
        },
        {
            x: new Date("1/4/2025"),
            y: 155,
        },
        {
            x: new Date("1/5/2025"),
            y: 160,
        },
        {
            x: new Date("1/6/2025"),
            y: 150,
        },
        {
            x: new Date("1/7/2025"),
            y: 142,
        },
        {
            x: new Date("1/8/2025"),
            y: 160,
        },
        {
            x: new Date("1/9/2025"),
            y: 148,
        },
    ]

    const dataB = [
        {
            x: new Date("1/5/2025"),
            y: 180,
        },
        {
            x: new Date("1/6/2025"),
            y: 186,
        },
        {
            x: new Date("1/7/2025"),
            y: 200,
        },
        {
            x: new Date("1/8/2025"),
            y: 175,
        },
        {
            x: new Date("1/9/2025"),
            y: 188,
        },
        {
            x: new Date("1/10/2025"),
            y: 195,
        },
        {
            x: new Date("1/11/2025"),
            y: 185,
        },
    ]

    const dataC = [
        {
            x: new Date("1/4/2025"),
            y: 120,
        },
        {
            x: new Date("1/5/2025"),
            y: 135,
        },
        {
            x: new Date("1/6/2025"),
            y: 115,
        },
        {
            x: new Date("1/7/2025"),
            y: 125,
        },
        {
            x: new Date("1/8/2025"),
            y: 130,
        },
    ]

    const chartOptions = {
        chart: {
            type: "area",
            height: 380,
            stacked: false,
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
            type: "datetime",
            labels: {
                datetimeUTC: false,
            },
        },
        yaxis: {
            min: 30,
        },
        tooltip: {
            x: {
                format: "dd MMM yyyy",
            },
            y: {
                formatter: (value) => value + " points",
            },
        },
        colors: ["#3e5eff", "#FFC700", "#FFAD84"],
        fill: {
            type: "gradient",
            gradient: {
                shadeIntensity: 1,
                opacityFrom: 0.8,
                opacityTo: 0.05,
                stops: [0, 100],
            },
        },
        stroke: {
            curve: "smooth",
            width: 2,
        },
        dataLabels: {
            enabled: false,
        },
        legend: {
            show: true,
            position: "bottom",
            offsetY: 6,
        },
        annotations: {
            yaxis: [
                {
                    y: 50,
                    borderColor: "#82A0D8",
                    label: {
                        text: "Support",
                        style: {
                            color: "#fff",
                            background: "#5356FF",
                        },
                    },
                },
            ],
            xaxis: [
                {
                    borderColor: "#82A0D8",
                    label: {
                        text: "Rally",
                        style: {
                            color: "#fff",
                            background: "#5356FF",
                        },
                    },
                },
            ],
        },
        series: [
            {
                name: "Product A",
                data: dataA,
            },
            {
                name: "Product B",
                data: dataB,
            },
            {
                name: "Product C",
                data: dataC,
            },
        ],
    }

    if (document.getElementById("irregular-time-area-chart")) {
        new ApexCharts(document.getElementById("irregular-time-area-chart"), chartOptions).render()
    }
}

const initSelectionAreaChart = () => {
    const data = [
        {
            x: 1722153590133,
            y: 50,
        },
        {
            x: 1722239990133,
            y: 62,
        },
        {
            x: 1722326390133,
            y: 67,
        },
        {
            x: 1722412790133,
            y: 87,
        },
        {
            x: 1722499190133,
            y: 101,
        },
        {
            x: 1722585590133,
            y: 118,
        },
        {
            x: 1722671990133,
            y: 136,
        },
        {
            x: 1722758390133,
            y: 135,
        },
        {
            x: 1722844790133,
            y: 125,
        },
        {
            x: 1722931190133,
            y: 114,
        },
        {
            x: 1723017590133,
            y: 105,
        },
        {
            x: 1723103990133,
            y: 113,
        },
        {
            x: 1723190390133,
            y: 112,
        },
        {
            x: 1723276790133,
            y: 113,
        },
        {
            x: 1723363190133,
            y: 98,
        },
        {
            x: 1723449590133,
            y: 92,
        },
        {
            x: 1723535990133,
            y: 75,
        },
        {
            x: 1723622390133,
            y: 71,
        },
        {
            x: 1723708790133,
            y: 52,
        },
        {
            x: 1723795190133,
            y: 71,
        },
        {
            x: 1723881590133,
            y: 54,
        },
        {
            x: 1723967990133,
            y: 51,
        },
        {
            x: 1724054390133,
            y: 57,
        },
        {
            x: 1724140790133,
            y: 62,
        },
        {
            x: 1724227190133,
            y: 80,
        },
        {
            x: 1724313590133,
            y: 95,
        },
        {
            x: 1724399990133,
            y: 100,
        },
        {
            x: 1724486390133,
            y: 101,
        },
        {
            x: 1724572790133,
            y: 120,
        },
        {
            x: 1724659190133,
            y: 104,
        },
        {
            x: 1724745590133,
            y: 98,
        },
        {
            x: 1724831990133,
            y: 116,
        },
        {
            x: 1724918390133,
            y: 98,
        },
        {
            x: 1725004790133,
            y: 118,
        },
        {
            x: 1725091190133,
            y: 113,
        },
        {
            x: 1725177590133,
            y: 122,
        },
        {
            x: 1725263990133,
            y: 141,
        },
        {
            x: 1725350390133,
            y: 145,
        },
        {
            x: 1725436790133,
            y: 160,
        },
        {
            x: 1725523190133,
            y: 151,
        },
        {
            x: 1725609590133,
            y: 165,
        },
        {
            x: 1725695990133,
            y: 154,
        },
        {
            x: 1725782390133,
            y: 141,
        },
        {
            x: 1725868790133,
            y: 127,
        },
        {
            x: 1725955190133,
            y: 147,
        },
        {
            x: 1726041590133,
            y: 142,
        },
        {
            x: 1726127990133,
            y: 148,
        },
        {
            x: 1726214390133,
            y: 166,
        },
        {
            x: 1726300790133,
            y: 163,
        },
        {
            x: 1726387190133,
            y: 147,
        },
        {
            x: 1726473590133,
            y: 134,
        },
        {
            x: 1726559990133,
            y: 121,
        },
        {
            x: 1726646390133,
            y: 129,
        },
        {
            x: 1726732790133,
            y: 124,
        },
        {
            x: 1726819190133,
            y: 111,
        },
        {
            x: 1726905590133,
            y: 112,
        },
        {
            x: 1726991990133,
            y: 110,
        },
        {
            x: 1727078390133,
            y: 95,
        },
        {
            x: 1727164790133,
            y: 99,
        },
        {
            x: 1727251190133,
            y: 82,
        },
        {
            x: 1727337590133,
            y: 69,
        },
        {
            x: 1727423990133,
            y: 74,
        },
        {
            x: 1727510390133,
            y: 61,
        },
        {
            x: 1727596790133,
            y: 74,
        },
        {
            x: 1727683190133,
            y: 85,
        },
        {
            x: 1727769590133,
            y: 74,
        },
        {
            x: 1727855990133,
            y: 92,
        },
        {
            x: 1727942390133,
            y: 73,
        },
        {
            x: 1728028790133,
            y: 59,
        },
        {
            x: 1728115190133,
            y: 54,
        },
        {
            x: 1728201590133,
            y: 53,
        },
        {
            x: 1728287990133,
            y: 65,
        },
        {
            x: 1728374390133,
            y: 53,
        },
        {
            x: 1728460790133,
            y: 69,
        },
        {
            x: 1728547190133,
            y: 61,
        },
        {
            x: 1728633590133,
            y: 65,
        },
        {
            x: 1728719990133,
            y: 63,
        },
        {
            x: 1728806390133,
            y: 66,
        },
        {
            x: 1728892790133,
            y: 67,
        },
        {
            x: 1728979190133,
            y: 56,
        },
        {
            x: 1729065590133,
            y: 61,
        },
        {
            x: 1729151990133,
            y: 62,
        },
        {
            x: 1729238390133,
            y: 50,
        },
        {
            x: 1729324790133,
            y: 69,
        },
        {
            x: 1729411190133,
            y: 82,
        },
        {
            x: 1729497590133,
            y: 79,
        },
        {
            x: 1729583990133,
            y: 99,
        },
        {
            x: 1729670390133,
            y: 101,
        },
        {
            x: 1729756790133,
            y: 120,
        },
        {
            x: 1729843190133,
            y: 134,
        },
        {
            x: 1729929590133,
            y: 151,
        },
        {
            x: 1730015990133,
            y: 152,
        },
        {
            x: 1730102390133,
            y: 148,
        },
        {
            x: 1730188790133,
            y: 130,
        },
        {
            x: 1730275190133,
            y: 123,
        },
        {
            x: 1730361590133,
            y: 119,
        },
        {
            x: 1730447990133,
            y: 107,
        },
        {
            x: 1730534390133,
            y: 102,
        },
        {
            x: 1730620790133,
            y: 90,
        },
        {
            x: 1730707190133,
            y: 91,
        },
        {
            x: 1730793590133,
            y: 101,
        },
        {
            x: 1730879990133,
            y: 91,
        },
        {
            x: 1730966390133,
            y: 99,
        },
        {
            x: 1731052790133,
            y: 92,
        },
        {
            x: 1731139190133,
            y: 105,
        },
        {
            x: 1731225590133,
            y: 96,
        },
        {
            x: 1731311990133,
            y: 93,
        },
        {
            x: 1731398390133,
            y: 78,
        },
        {
            x: 1731484790133,
            y: 69,
        },
        {
            x: 1731571190133,
            y: 51,
        },
        {
            x: 1731657590133,
            y: 52,
        },
        {
            x: 1731743990133,
            y: 65,
        },
        {
            x: 1731830390133,
            y: 52,
        },
        {
            x: 1731916790133,
            y: 70,
        },
        {
            x: 1732003190133,
            y: 90,
        },
        {
            x: 1732089590133,
            y: 91,
        },
        {
            x: 1732175990133,
            y: 88,
        },
        {
            x: 1732262390133,
            y: 75,
        },
        {
            x: 1732348790133,
            y: 67,
        },
        {
            x: 1732435190133,
            y: 70,
        },
        {
            x: 1732521590133,
            y: 69,
        },
        {
            x: 1732607990133,
            y: 56,
        },
        {
            x: 1732694390133,
            y: 51,
        },
        {
            x: 1732780790133,
            y: 65,
        },
        {
            x: 1732867190133,
            y: 69,
        },
        {
            x: 1732953590133,
            y: 65,
        },
        {
            x: 1733039990133,
            y: 68,
        },
        {
            x: 1733126390133,
            y: 73,
        },
        {
            x: 1733212790133,
            y: 88,
        },
        {
            x: 1733299190133,
            y: 100,
        },
        {
            x: 1733385590133,
            y: 112,
        },
        {
            x: 1733471990133,
            y: 122,
        },
        {
            x: 1733558390133,
            y: 131,
        },
        {
            x: 1733644790133,
            y: 133,
        },
        {
            x: 1733731190133,
            y: 130,
        },
        {
            x: 1733817590133,
            y: 136,
        },
        {
            x: 1733903990133,
            y: 128,
        },
        {
            x: 1733990390133,
            y: 136,
        },
        {
            x: 1734076790133,
            y: 145,
        },
        {
            x: 1734163190133,
            y: 149,
        },
        {
            x: 1734249590133,
            y: 150,
        },
        {
            x: 1734335990133,
            y: 153,
        },
        {
            x: 1734422390133,
            y: 171,
        },
        {
            x: 1734508790133,
            y: 152,
        },
        {
            x: 1734595190133,
            y: 163,
        },
        {
            x: 1734681590133,
            y: 150,
        },
        {
            x: 1734767990133,
            y: 149,
        },
        {
            x: 1734854390133,
            y: 141,
        },
        {
            x: 1734940790133,
            y: 161,
        },
        {
            x: 1735027190133,
            y: 177,
        },
        {
            x: 1735113590133,
            y: 178,
        },
        {
            x: 1735199990133,
            y: 178,
        },
        {
            x: 1735286390133,
            y: 184,
        },
        {
            x: 1735372790133,
            y: 193,
        },
    ]

    const resultChartOptions = {
        series: [
            {
                name: "Visitors",
                data: data,
            },
        ],
        chart: {
            id: "result-chart",
            height: 190,
            type: "area",
            background: "transparent",
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
        colors: ["#FDA403"],
        stroke: {
            width: 2,
            curve: "smooth",
        },
        dataLabels: {
            enabled: false,
        },
        fill: {
            opacity: 0.6,
            type: "solid",
        },
        xaxis: {
            type: "datetime",
        },
    }

    const targetChartOptions = {
        series: [
            {
                name: "Visitors",
                data: data,
            },
        ],
        chart: {
            height: 190,
            type: "area",
            background: "transparent",
            toolbar: {
                show: true,
                tools: {
                    download: true,
                    selection: true,
                    zoom: false,
                    zoomin: false,
                    zoomout: false,
                    pan: false,
                    reset: false,
                },
                autoSelected: "selection",
            },
            brush: {
                enabled: true,
                target: "result-chart",
            },
            selection: {
                enabled: true,
                fill: {
                    color: "#FDA403",
                    opacity: 0.3,
                },
                stroke: {
                    width: 2,
                    color: "#FDA403",
                    opacity: 0.8,
                    dashArray: 3,
                },
                xaxis: {
                    min: 1726041590133,
                    max: 1726041590133 + 15 * 60 * 1000 * 1000,
                },
            },
        },
        colors: ["#167bff"],
        dataLabels: {
            enabled: false,
        },
        stroke: {
            width: 2,
            curve: "smooth",
        },
        fill: {
            opacity: 0.6,
            type: "solid",
        },
        xaxis: {
            type: "datetime",
        },
    }

    if (document.getElementById("selection-result-area-chart")) {
        new ApexCharts(
            document.getElementById("selection-result-area-chart"),
            resultChartOptions
        ).render()
    }

    if (document.getElementById("selection-target-area-chart")) {
        new ApexCharts(
            document.getElementById("selection-target-area-chart"),
            targetChartOptions
        ).render()
    }
}

document.addEventListener("DOMContentLoaded", function () {
    initSplineChart()
    initNegativeValueChart()
    initIrregularTimeSeriesChart()
    initSelectionAreaChart()
})
