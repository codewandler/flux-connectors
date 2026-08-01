op babelforce-revoke(client_id: String, client_secret: String, token: String, token_type_hint: String) -> Any
  description "OAuth 2.0 token revocation (RFC 7009)"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/oauth/revoke")
  content_type = "application/x-www-form-urlencoded"
  payload = fmt("token={token}")
  when client_id
    payload = fmt("{payload}&client_id={client_id}")
  when client_secret
    payload = fmt("{payload}&client_secret={client_secret}")
  when token_type_hint
    payload = fmt("{payload}&token_type_hint={token_type_hint}")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
