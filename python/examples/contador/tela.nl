<nativelayout>
<body>
  <h1>VPN</h1>
  <div>
    <p>{status_label}</p>
    <p>{status_ip}</p>
    <p>{status_loc}</p>
  </div>
  <div id="btn_connect">{btn_text}</div>
  <p>Servidores</p>
  <for each="servers">
    <div id="server">
      <p>{flag} {name} - {ping}</p>
    </div>
  </for>
</body>
</nativelayout>