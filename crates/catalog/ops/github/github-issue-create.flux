op github-issue-create(owner: String, repo: String, title: String, body: String, labels: List<String>, assignees: List<String>) -> Any
  description "Open a new issue on a repository. The issue is immediately visible to everyone who can see the repository and notifies its subscribers"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/issues")
  content_type = "application/json"
  Accept = "application/vnd.github+json"
  payload = { assignees, body, labels, title }
  response = http.request(body: payload, headers: { Accept, "content-type": content_type }, method: "POST", url)
  return response
