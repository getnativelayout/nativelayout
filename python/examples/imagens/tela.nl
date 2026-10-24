<nativelayout>
<body>
  <header>
    <h1>VPN</h1>
  </header>
  <div>
    <p>{status_label}</p>
    <h2>{status_ip}</h2>
    <p>{status_loc}</p>
  </div>
  <button id="btn_connect">{btn_text}</button>
  <h3>Servidores</h3>
  for s in servers:
    <div id="server" key="{s.id}">
      <p>{s.name} - {s.ping}</p>
    </div>
  end
</body>
</nativelayout>