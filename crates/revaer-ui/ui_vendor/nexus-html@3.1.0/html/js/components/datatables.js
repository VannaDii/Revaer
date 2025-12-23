const tableData = [
    {
        id: "a1b2c3d4",
        customerName: "Alice Johnson",
        amount: 120,
        status: "success",
        email: "alice.j@mail.com",
        avatar: "/images/avatars/1.png",
    },
    {
        id: "e5f6g7h8",
        customerName: "Bob Smith",
        amount: 250,
        status: "pending",
        email: "bobsmith@outlk.com",
        avatar: "/images/avatars/2.png",
    },
    {
        id: "i9j0k1l2",
        customerName: "Charlie Davis",
        amount: 75,
        status: "failed",
        email: "charl.davis@ex.com",
        avatar: "/images/avatars/3.png",
    },
    {
        id: "m3n4o5p6",
        customerName: "Diana Cruz",
        amount: 300,
        status: "processing",
        email: "diana.c@gnail.com",
        avatar: "/images/avatars/4.png",
    },
    {
        id: "q7r8s9t0",
        customerName: "Ethan Ford",
        amount: 190,
        status: "success",
        email: "ethan.f@fastmal.com",
        avatar: "/images/avatars/5.png",
    },
    {
        id: "u1v2w3x4",
        customerName: "Fiona Green",
        amount: 220,
        status: "pending",
        email: "fiona.g@yaho.com",
        avatar: "/images/avatars/6.png",
    },
    {
        id: "y5z6a7b8",
        customerName: "George Hill",
        amount: 180,
        status: "failed",
        email: "george.h@mail.com",
        avatar: "/images/avatars/7.png",
    },
    {
        id: "c9d0e1f2",
        customerName: "Hannah Lee",
        amount: 140,
        status: "processing",
        email: "hannah.l@ex.org",
        avatar: "/images/avatars/8.png",
    },
    {
        id: "g3h4i5j6",
        customerName: "Ian Martinez",
        amount: 160,
        status: "success",
        email: "ian.mart@domain.com",
        avatar: "/images/avatars/9.png",
    },
    {
        id: "k7l8m9n0",
        customerName: "Jasmine King",
        amount: 275,
        status: "success",
        email: "jasm.k@outlk.com",
        avatar: "/images/avatars/10.png",
    },
    {
        id: "o1p2q3r4",
        customerName: "Kyle Young",
        amount: 130,
        status: "pending",
        email: "kyle.y@mail.com",
        avatar: "/images/avatars/1.png",
    },
    {
        id: "s5t6u7v8",
        customerName: "Lara Scott",
        amount: 210,
        status: "processing",
        email: "lara.s@gmail.com",
        avatar: "/images/avatars/2.png",
    },
    {
        id: "w9x0y1z2",
        customerName: "Mike Turner",
        amount: 85,
        status: "failed",
        email: "mike.t@ex.com",
        avatar: "/images/avatars/3.png",
    },
    {
        id: "a3b4c5d6",
        customerName: "Nina Adams",
        amount: 145,
        status: "success",
        email: "nina.a@domain.org",
        avatar: "/images/avatars/4.png",
    },
    {
        id: "e7f8g9h0",
        customerName: "Oscar Perez",
        amount: 205,
        status: "processing",
        email: "oscar.p@mail.com",
        avatar: "/images/avatars/5.png",
    },
    {
        id: "i1j2k3l4",
        customerName: "Paula Reed",
        amount: 170,
        status: "pending",
        email: "paula.r@yaoo.com",
        avatar: "/images/avatars/6.png",
    },
    {
        id: "m5n6o7p8",
        customerName: "Quinn Blake",
        amount: 125,
        status: "failed",
        email: "quinn.b@domain.com",
        avatar: "/images/avatars/7.png",
    },
    {
        id: "q9r0s1t2",
        customerName: "Rachel Nguyen",
        amount: 195,
        status: "success",
        email: "rachel.n@gmail.com",
        avatar: "/images/avatars/8.png",
    },
    {
        id: "u3v4w5x6",
        customerName: "Steve White",
        amount: 155,
        status: "processing",
        email: "steve.w@fastml.com",
        avatar: "/images/avatars/9.png",
    },
    {
        id: "y7z8a9b0",
        customerName: "Tina Brown",
        amount: 110,
        status: "pending",
        email: "tina.b@mail.com",
        avatar: "/images/avatars/10.png",
    },
    {
        id: "c1d2e3f4",
        customerName: "Umar Khan",
        amount: 240,
        status: "success",
        email: "umar.k@outlk.com",

        avatar: "/images/avatars/1.png",
    },
    {
        id: "g5h6i7j8",
        customerName: "Vera Lopez",
        amount: 185,
        status: "failed",
        email: "vera.l@ex.org",

        avatar: "/images/avatars/2.png",
    },
    {
        id: "k9l0m1n2",
        customerName: "Will Grant",
        amount: 200,
        status: "processing",
        email: "will.g@gmail.com",

        avatar: "/images/avatars/3.png",
    },
    {
        id: "o3p4q5r6",
        customerName: "Xena Foster",
        amount: 95,
        status: "pending",
        email: "xena.f@domain.com",

        avatar: "/images/avatars/4.png",
    },
    {
        id: "s7t8u9v0",
        customerName: "Yara Brooks",
        amount: 260,
        status: "success",
        email: "yara.b@mail.com",

        avatar: "/images/avatars/5.png",
    },
    {
        id: "w1x2y3z4",
        customerName: "Zane Cooper",
        amount: 150,
        status: "failed",
        email: "zane.c@outlk.com",
        avatar: "/images/avatars/6.png",
    },
    {
        id: "a5b6c7d8",
        customerName: "Amy Flynn",
        amount: 170,
        status: "processing",
        email: "amy.f@protonml.com",
        avatar: "/images/avatars/7.png",
    },
    {
        id: "e9f0g1h2",
        customerName: "Ben Knight",
        amount: 230,
        status: "pending",
        email: "ben.k@ex.com",
        avatar: "/images/avatars/8.png",
    },
    {
        id: "i3j4k5l6",
        customerName: "Cathy Holt",
        amount: 215,
        status: "success",
        email: "cathy.h@domain.org",
        avatar: "/images/avatars/9.png",
    },
    {
        id: "m7n8o9p0",
        customerName: "Dan Rivera",
        amount: 165,
        status: "processing",
        email: "dan.r@gmail.com",
        avatar: "/images/avatars/10.png",
    },
    {
        id: "q1r2s3t4",
        customerName: "Elle Baxter",
        amount: 105,
        status: "failed",
        email: "elle.b@mail.com",

        avatar: "/images/avatars/1.png",
    },
    {
        id: "u5v6w7x8",
        customerName: "Frank Moore",
        amount: 250,
        status: "success",
        email: "frank.m@domain.com",
        avatar: "/images/avatars/2.png",
    },
    {
        id: "y9z0a1b2",
        customerName: "Grace Owen",
        amount: 135,
        status: "pending",
        email: "grace.o@fastml.com",
        avatar: "/images/avatars/3.png",
    },
    {
        id: "c3d4e5f6",
        customerName: "Henry Webb",
        amount: 200,
        status: "processing",
        email: "henry.w@ex.com",
        avatar: "/images/avatars/4.png",
    },
    {
        id: "g7h8i9j0",
        customerName: "Isla Bennett",
        amount: 115,
        status: "failed",
        email: "isla.b@outlk.com",
        avatar: "/images/avatars/5.png",
    },
    {
        id: "k1l2m3n4",
        customerName: "Jake Sims",
        amount: 185,
        status: "success",
        email: "jake.s@gmail.com",
        avatar: "/images/avatars/6.png",
    },
    {
        id: "o5p6q7r8",
        customerName: "Kara Diaz",
        amount: 145,
        status: "processing",
        email: "kara.d@domain.org",
        avatar: "/images/avatars/7.png",
    },
    {
        id: "s9t0u1v2",
        customerName: "Leo Payne",
        amount: 175,
        status: "pending",
        email: "leo.p@mail.com",
        avatar: "/images/avatars/8.png",
    },
    {
        id: "w3x4y5z6",
        customerName: "Maya Dean",
        amount: 210,
        status: "success",
        email: "maya.d@outlk.com",
        avatar: "/images/avatars/9.png",
    },
    {
        id: "a7b8c9d0",
        customerName: "Noah Gibbs",
        amount: 190,
        status: "failed",
        email: "noah.g@domain.com",
        avatar: "/images/avatars/10.png",
    },
    {
        id: "e1f2g3h4",
        customerName: "Olivia Nash",
        amount: 155,
        status: "processing",
        email: "olivia.n@gmail.com",

        avatar: "/images/avatars/1.png",
    },
    {
        id: "i5j6k7l8",
        customerName: "Paul Clark",
        amount: 140,
        status: "pending",
        email: "paul.c@ex.com",

        avatar: "/images/avatars/2.png",
    },
    {
        id: "m9n0o1p2",
        customerName: "Rita Fox",
        amount: 165,
        status: "success",
        email: "rita.f@domain.org",

        avatar: "/images/avatars/3.png",
    },
    {
        id: "q3r4s5t6",
        customerName: "Sam Walsh",
        amount: 235,
        status: "failed",
        email: "sam.w@fastml.com",
        avatar: "/images/avatars/4.png",
    },
    {
        id: "u7v8w9x0",
        customerName: "Tara Neal",
        amount: 100,
        status: "processing",
        email: "tara.n@mail.com",
        avatar: "/images/avatars/5.png",
    },
    {
        id: "y1z2a3b4",
        customerName: "Victor Lang",
        amount: 180,
        status: "pending",
        email: "victor.l@outlk.com",
        avatar: "/images/avatars/6.png",
    },
    {
        id: "c5d6e7f8",
        customerName: "Wendy Cross",
        amount: 220,
        status: "success",
        email: "wendy.c@domain.com",
        avatar: "/images/avatars/7.png",
    },
    {
        id: "u1v2w3x4",
        customerName: "Lena Matthews",
        amount: 220,
        status: "success",
        email: "lena.m@outlk.com",
        avatar: "/images/avatars/8.png",
    },
    {
        id: "y5z6a7b8",
        customerName: "Marcus Lee",
        amount: 145,
        status: "failed",
        email: "marcus.l@protonml.com",
        avatar: "/images/avatars/9.png",
    },
    {
        id: "c9d0e1f2",
        customerName: "Nora Patel",
        amount: 310,
        status: "processing",
        email: "nora.p@mail.com",
        avatar: "/images/avatars/10.png",
    },
].map((data) => {
    return {
        ...data,
        dateTime: new Date(Date.now() - 1000 * 60 * 60 * Math.floor(Math.random() * 24 * 100)),
    }
})

const getTableData = () => {
    return [...tableData].sort(() => 0.5 - Math.random())
}

const flexRender = (comp, props) => {
    if (typeof comp === "function") {
        return comp(props)
    }
    return comp
}

// Tables
window.useSimpleDatatables = () => {
    const data = getTableData()
    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: { pageSize: 5, pageIndex: 0 },
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
    })
    return {
        table,
        flexRender,
    }
}

window.useAdvancedDatatables = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
        { accessorKey: "actions", header: "Actions" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 10,
            pageIndex: 0,
        },
        globalFilter: "",
        columnFilters: [],
        columnVisibility: {},
        rowSelection: {},
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        getFilteredRowModel: TableCore.getFilteredRowModel(),
        globalFilterFn: "auto",
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })
    return {
        version,
        columns,
        pageSizes: [5, 10, 20, 50],
        flexRender,
        search: "",
        get table() {
            this.version
            return table
        },
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        get selectedCount() {
            return this.table.getSelectedRowModel().rows.length
        },
        get totalCount() {
            return this.table.getPaginationRowModel().rows.length
        },
        get isIndeterminateAllRowsSelected() {
            this.version
            return this.table.getIsSomePageRowsSelected() && !this.table.getIsAllPageRowsSelected()
        },
        get allLeafColumns() {
            this.version
            return this.table.getAllLeafColumns()
        },
        get pageSize() {
            this.version
            return this.table.getState().pagination.pageSize
        },
        get pageIndex() {
            this.version
            return this.table.getState().pagination.pageIndex
        },
        get rowCount() {
            this.version
            return data.length
        },
        get start() {
            this.version
            return this.rowCount === 0 ? 0 : this.pageIndex * this.pageSize + 1
        },
        get end() {
            this.version
            return Math.min(this.start + this.pageSize - 1, this.rowCount)
        },

        setPageIndex(n) {
            this.table.setPageIndex(n)
            this.render()
        },

        nextPage() {
            this.version
            if (this.table.getCanNextPage()) {
                this.table.setPageIndex(this.pageIndex + 1)
                this.render()
            }
        },

        prevPage() {
            this.version
            if (this.table.getCanPreviousPage()) {
                this.table.setPageIndex(this.pageIndex - 1)
                this.render()
            }
        },

        changePageSize(newSize) {
            this.table.setPageSize(Number(newSize))
            this.render()
        },
        updateSearch() {
            table.setState({
                ...table.getState(),
                globalFilter: this.search,
            })
            this.render()
        },
        getVisibleCells(row) {
            this.version
            return row.getVisibleCells()
        },
        isColumnVisible(column) {
            this.version
            return column.getIsVisible()
        },
        toggleColumn(column) {
            column.toggleVisibility()
            this.render()
        },
        toggleSelectedRow(row) {
            row.toggleSelected()
            this.render()
        },
        isRowSelected(row) {
            this.version
            return row.getIsSelected()
        },
        toggleAllRowsSelected() {
            this.table.toggleAllPageRowsSelected()
            this.render()
        },
        viewRow(row) {
            alert(`View #${row.original.id}`)
        },
        deleteRow(row) {
            alert(`Delete #${row.original.id}`)
        },
        clearFilters() {
            this.search = ""
            this.updateSearch()
        },
        render() {
            this.version++
        },
    }
}

window.useColumnSearchDatatables = () => {
    const data = getTableData()
    let version = 0
    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]
    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
    }
    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        getFilteredRowModel: TableCore.getFilteredRowModel(),
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })
    return {
        table,
        version,
        columns,
        flexRender,
        search: "",
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        updateSearch() {
            this.table.getColumn("customerName")?.setFilterValue(this.search)
            this.version++
        },
        clearFilters() {
            this.search = ""
            this.updateSearch()
        },
    }
}

window.useColumnVisibilityTable = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
        columnVisibility: {},
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })

    return {
        columns,
        version,
        flexRender,
        get table() {
            this.version
            return table
        },
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        getVisibleCells(row) {
            this.version
            return row.getVisibleCells()
        },
        get allLeafColumns() {
            this.version
            return this.table.getAllLeafColumns()
        },
        isColumnVisible(column) {
            this.version
            return column.getIsVisible()
        },
        toggleColumn(column) {
            column.toggleVisibility()
            this.render()
        },
        render() {
            this.version++
        },
    }
}

window.useGlobalSearchDatatables = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
        globalFilter: "",
        columnFilters: [],
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        getFilteredRowModel: TableCore.getFilteredRowModel(),
        globalFilterFn: "auto",
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })
    return {
        table,
        version,
        columns,
        flexRender,
        search: "",
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        updateSearch() {
            table.setState({
                ...table.getState(),
                globalFilter: this.search,
            })
            this.render()
        },
        clearFilters() {
            this.search = ""
            this.updateSearch()
        },
        render() {
            this.version++
        },
    }
}

window.usePaginatedTable = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })

    return {
        columns,
        flexRender,
        pageSizes: [5, 10, 20, 50],
        version,
        get table() {
            this.version
            return table
        },
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        get pageSize() {
            this.version
            return this.table.getState().pagination.pageSize
        },
        get pageIndex() {
            this.version
            return this.table.getState().pagination.pageIndex
        },
        get rowCount() {
            this.version
            return data.length
        },
        get start() {
            this.version
            return this.rowCount === 0 ? 0 : this.pageIndex * this.pageSize + 1
        },
        get end() {
            this.version
            return Math.min(this.start + this.pageSize - 1, this.rowCount)
        },

        setPageIndex(n) {
            this.table.setPageIndex(n)
            this.render()
        },

        nextPage() {
            this.version
            if (this.table.getCanNextPage()) {
                this.table.setPageIndex(this.pageIndex + 1)
                this.render()
            }
        },

        prevPage() {
            this.version
            if (this.table.getCanPreviousPage()) {
                this.table.setPageIndex(this.pageIndex - 1)
                this.render()
            }
        },

        changePageSize(newSize) {
            this.table.setPageSize(Number(newSize))
            this.render()
        },

        render() {
            this.version++
        },
    }
}

window.useRowActionsDatatables = () => {
    const data = getTableData()
    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
        {
            accessorKey: "actions",
            header: "Actions",
        },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: { pageSize: 5, pageIndex: 0 },
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
    })
    return {
        table,
        flexRender,
        viewRow(row) {
            alert(`View #${row.original.id}`)
        },
        deleteRow(row) {
            alert(`Delete #${row.original.id}`)
        },
    }
}

window.useRowSelectionTable = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
        rowSelection: {},
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })

    return {
        columns,
        flexRender,
        version,
        get table() {
            this.version
            return table
        },
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        get selectedCount() {
            return this.table.getSelectedRowModel().rows.length
        },
        get totalCount() {
            return this.table.getPaginationRowModel().rows.length
        },
        get isIndeterminateAllRowsSelected() {
            this.version
            return this.table.getIsSomePageRowsSelected() && !this.table.getIsAllPageRowsSelected()
        },
        toggleSelectedRow(row) {
            row.toggleSelected()
            this.render()
        },
        isRowSelected(row) {
            this.version
            return row.getIsSelected()
        },
        toggleAllRowsSelected() {
            this.table.toggleAllPageRowsSelected()
            this.render()
        },
        render() {
            this.version++
        },
    }
}

window.useScrollableDatatables = () => {
    const data = getTableData()

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
    })
    return {
        table,
        flexRender,
    }
}

window.useSortingTable = () => {
    const data = getTableData()
    let version = 0

    const columns = [
        { accessorKey: "id", header: "ID" },
        { accessorKey: "customerName", header: "Customer" },
        { accessorKey: "status", header: "Status" },
        { accessorKey: "amount", header: "Amount" },
        { accessorKey: "dateTime", header: "Order At" },
    ]

    const state = {
        columnPinning: { left: [], right: [] },
        pagination: {
            pageSize: 5,
            pageIndex: 0,
        },
    }

    const table = TableCore.createTable({
        state,
        data,
        columns,
        getCoreRowModel: TableCore.getCoreRowModel(),
        getPaginationRowModel: TableCore.getPaginationRowModel(),
        getSortedRowModel: TableCore.getSortedRowModel(),
        onStateChange: (updater) => {
            const newState = typeof updater === "function" ? updater(state) : updater
            Object.assign(state, newState)
        },
    })

    return {
        columns,
        flexRender,
        version,
        get table() {
            this.version
            return table
        },
        get visibleRows() {
            this.version
            return this.table.getRowModel().rows
        },
        toggleColumnSorting(column) {
            column.toggleSorting(column.getIsSorted() === "asc")
            this.render()
        },
        isSorted(column) {
            this.version
            return column.getIsSorted()
        },
        render() {
            this.version++
        },
    }
}
