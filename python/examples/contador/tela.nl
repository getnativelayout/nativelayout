<nativelayout>
<style>
:root { --azul: #2F81F7; }
body { padding: 16px; gap: 12px; background-color: #F6F8FA; }
h1 { color: var(--azul); }
.card { background-color: white; border-radius: 12px; padding: 12px; box-shadow: 0 2px 6px rgba(0,0,0,0.3); gap: 8px; }
.row { display: flex; flex-direction: row; gap: 8px; align-items: center; }
.mais { background-color: var(--azul); color: white; border-radius: 20px; }
</style>
<body>
<header style="background-color:#24292F;padding:12px;"><p style="color:white;margin:0;font-weight:bold">Contador</p></header>
<h1>Ola, {nome}</h1>
<div class="card">
<p style="margin:0">Total: <span>{n}</span></p>
<div class="row">
<button class="mais" style="flex:1">+ somar</button>
<img src="icon:star" style="color:#D29922" />
<input type="checkbox" checked />
</div>
<input type="text" id="campo" placeholder="escreve aqui" />
</div>
<ul>
for t in tarefas:
<li>{t}</li>
end
</ul>
<footer style="background-color:#24292F;padding:10px;justify-content:center"><p style="color:white;margin:0">rodape</p></footer>
</body>
</nativelayout>