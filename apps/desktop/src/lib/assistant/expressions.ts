import content from "../../assets/avatars/content.png";
import smile from "../../assets/avatars/smile.png";
import questioning from "../../assets/avatars/questioning.png";
import laughing from "../../assets/avatars/laughing.png";
import confused from "../../assets/avatars/confused.png";

export type AssistantState =
  | "idle"
  | "listening"
  | "thinking"
  | "speaking"
  | "confused";

export const expressionFor = (state: AssistantState): string => {
  switch (state) {
    case "idle":
      return content;
    case "listening":
      return smile;
    case "thinking":
      return questioning;
    case "speaking":
      return smile;
    case "confused":
      return confused;
  }
};

// Reserved — triggered by delight heuristic in BubbleLayer, not by AssistantState.
export const LAUGHING = laughing;
