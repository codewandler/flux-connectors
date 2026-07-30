op github-pull-get(owner: String, repo: String, pull_number: Number) -> Any
  description "Get one pull request by number, with its merge state, head and base refs and review counts"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://api.github.com"
  $url = fmt("{base}/repos/{owner}/{repo}/pulls/{pull_number}")
  $response = http.request({ method: "GET", url: $url })
  return $response
