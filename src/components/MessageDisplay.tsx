import { useEffect, useState } from "react";

type MessageDisplayProps = {
  message: string | null;
  isTemporary: boolean;
  clearMessage: () => void;
};

const MessageDisplay: React.FC<MessageDisplayProps> = ({
  message,
  isTemporary,
  clearMessage,
}) => {
  const [isVisible, setIsVisible] = useState<boolean>(false);

  useEffect(() => {
    if (message) {
      setIsVisible(true);
      if (isTemporary) {
        const timeout = setTimeout(() => {
          setIsVisible(false);
          setTimeout(() => clearMessage(), 500);
        }, 2000);

        return () => clearTimeout(timeout);
      }
    }
  }, [isTemporary, message, clearMessage]);

  return (
    <div className={`editor-message ${isVisible ? "fade-in" : "fade-out"}`}>
      <span>{message || ""}</span>
    </div>
  );
};

export default MessageDisplay;
