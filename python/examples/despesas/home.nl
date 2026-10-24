<nativelayout>
<link rel="stylesheet" href="theme.css">

<body>
    <header>
        <h1 class="apptitle">Despesas</h1>
    </header>

    <div class="page">
        <div class="card">
            <p class="muted">Total</p>
            <h2>{total}</h2>
            <p class="muted">{count} despesas</p>
        </div>

        <button id="add" class="primary">+ Adicionar despesa</button>

        <div class="chips">
            for c in chips:
                if c.state == "on":
                    <button class="chip on" key="{c.id}">{c.name}</button>
                else:
                    <button class="chip" key="{c.id}">{c.name}</button>
                end
            end
        </div>

        if count > 0:
            <h3>Por categoria</h3>
            for s in summary:
                <div class="cat" key="{s.id}">
                    <div class="split">
                        <p>{s.name}</p>
                        <p class="muted">{s.amount}</p>
                    </div>
                    <div class="track"><div class="fill" style="width: {s.percent}%"></div></div>
                </div>
            end

            <h3>Mais recentes</h3>
            for e in expenses:
                <include src="expense.nl" title="{e.title}" category="{e.category}" amount="{e.amount}">
            end
        else:
            <p class="empty">Ainda não há despesas aqui. Toque em "Adicionar despesa" para criar a primeira.</p>
        end
    </div>

    if confirm != "":
        <dialog id="layer" style="justify-content: flex-end; align-items: stretch">
            <div class="sheet">
                <p class="sheet-title">Apagar despesa?</p>
                <p class="sheetmsg">{dialogText}</p>
                <div class="actions">
                    <button id="confirm-no" class="ghost dlg">Cancelar</button>
                    <button id="confirm-yes" class="danger dlg">Apagar</button>
                </div>
            </div>
        </dialog>
    end
</body>

</nativelayout>
