import type { Messages } from "./en";

// Korean is the second locale. Keep wording short and natural; it is
// reviewed separately from the English source.
export const ko: Messages = {
  "status.working": "작업 중",
  "status.blocked.permission": "승인 대기",
  "status.blocked.question": "답변 대기",
  "status.blocked.auth": "로그인 필요",
  "status.idle": "입력 대기",
  "status.error": "오류",
  "status.exited": "종료",
  "status.crashed": "비정상 종료",
  "status.unknown": "확인 중",

  "group.needs": "확인이 필요해요",
  "group.working": "작업 중",
  "group.review": "검토할 작업",
  "group.done": "끝난 작업",

  "titlebar.search": "작업, 파일, 명령 찾기",
  "titlebar.view": "보기",
  "view.tasks": "작업",
  "view.board": "보드",
  "titlebar.notifications": "알림",
  "titlebar.settings": "설정",

  "sidebar.title": "작업",
  "sidebar.newTask": "새 작업",
  "sidebar.empty.title": "아직 작업이 없어요",
  "sidebar.empty.body": "작업을 만들면 전용 worktree에서 에이전트가 시작돼요.",

  "task.reviewChanges": "변경 검토",
  "task.createPr": "PR 만들기",
  "task.openInEditor": "편집기로 열기",
  "task.newTab": "새 탭",
  "task.split": "나누기",
  "task.more": "더 보기",

  "approval.title": "이 명령을 실행할까요?",
  "approval.inWorktree": "worktree 안에서 실행",
  "approval.allowOnce": "한 번 허용",
  "approval.allowAlways": "이 작업에서는 항상 허용",
  "approval.deny": "거부",
  "approval.hint": "터미널에서 바로 답해도 돼요.",

  "inspector.changes": "변경",
  "inspector.qa": "QA",
  "inspector.activity": "활동",
  "inspector.changedFiles": "바뀐 파일",
  "inspector.qa.passed": "통과",
  "inspector.qa.failed": "실패",
  "inspector.qa.none": "이 작업은 아직 QA를 돌리지 않았어요.",
  "inspector.activity.none": "에이전트가 일하는 동안 여기에 기록이 쌓여요.",

  "board.title": "보드",
  "board.all": "전체",
  "board.open": "열기",

  "statusbar.core": "코어 정상",
  "statusbar.sessions": "세션",
  "statusbar.memory": "메모리",
  "statusbar.cpu": "CPU",

  "empty.repo.title": "저장소를 추가해서 시작해요",
  "empty.repo.body": "git 저장소 폴더를 고르거나 이 창에 끌어다 놓으세요.",
  "empty.repo.add": "저장소 추가",

  "time.now": "방금",
};
