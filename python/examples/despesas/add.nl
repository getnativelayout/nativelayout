<nativelayout>
<link rel="stylesheet" href="theme.css">

<body>
    <header>
        <img id="cancel" class="ibtn" src="icon:arrow-left" alt="Voltar">
        <h1 class="apptitle">Nova despesa</h1>
    </header>

    <div class="page">
        <fieldset>
            <legend>Detalhes</legend>
            <input id="title" type="text" placeholder="O que foi?">
            <input id="amount" type="number" placeholder="Valor">
            <p class="muted">Categoria</p>
            <div class="chips">
                for c in categories:
                    if c.state == "on":
                        <button class="chip on" key="{c.id}">{c.name}</button>
                    else:
                        <button class="chip" key="{c.id}">{c.name}</button>
                    end
                end
            </div>
            if error != "":
                <p class="error">{error}</p>
            end
        </fieldset>

        <button id="save" class="primary">Guardar</button>
    </div>
</body>

</nativelayout>
