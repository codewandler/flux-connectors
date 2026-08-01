op babelforce-token(access_type: String, client_id: String, client_secret: String, code: String, code_verifier: String, grant_type: String, password: String, redirect_uri: String, refresh_token: String, scope: String, username: String) -> Any
  description "OAuth 2.0 token endpoint"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/oauth/token")
  content_type = "application/x-www-form-urlencoded"
  payload = fmt("grant_type={grant_type}")
  when access_type
    payload = fmt("{payload}&access_type={access_type}")
  when client_id
    payload = fmt("{payload}&client_id={client_id}")
  when client_secret
    payload = fmt("{payload}&client_secret={client_secret}")
  when code
    payload = fmt("{payload}&code={code}")
  when code_verifier
    payload = fmt("{payload}&code_verifier={code_verifier}")
  when password
    payload = fmt("{payload}&password={password}")
  when redirect_uri
    payload = fmt("{payload}&redirect_uri={redirect_uri}")
  when refresh_token
    payload = fmt("{payload}&refresh_token={refresh_token}")
  when $scope
    payload = fmt("{payload}&scope={scope}")
  when username
    payload = fmt("{payload}&username={username}")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
