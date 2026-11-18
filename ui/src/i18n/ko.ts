import type { Messages } from "./en";

// Korean is the second locale. Keep wording short and natural; it is
// reviewed separately from the English source.
export const ko: Messages = {
  "status.working": "작업 중",
  "status.blocked.permission": "승인 대기",
  "status.blocked.question": "답변 대기",
  "status.blocked.auth": "로그인 필요",
  "status.blocked.other": "입력 기다림",
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
  "terminal.exited": "프로세스가 끝났어요",
  "terminal.startFailed": "터미널을 시작하지 못했어요",

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

  "newTask.title": "새 작업",
  "newTask.prompt": "에이전트에게 무엇을 맡길까요?",
  "newTask.name": "작업 이름",
  "newTask.base": "기준 브랜치",
  "newTask.agent": "에이전트",
  "newTask.cancel": "취소",
  "newTask.create": "작업 만들기",
  "newTask.creating": "만드는 중...",
  "empty.task.title": "고른 작업이 없어요",
  "empty.task.body": "작업을 만들면 전용 worktree에서 에이전트가 시작돼요.",
  "error.dismiss": "닫기",
  "error.title": "문제가 생겼어요",

  "notify.permission": "승인이 필요해요",
  "notify.question": "질문이 있어요",
  "notify.auth": "로그인이 필요해요",
  "notify.other": "입력을 기다려요",
  "notify.error": "문제가 생겼어요",
  "notify.finished": "끝났어요",
  "notify.crashed": "비정상 종료됐어요",

  "time.now": "방금",
};
