import { useCallback, useEffect, useRef, useState } from "react";

export interface Position {
  x: number;
  y: number;
}

export interface UseDraggableWindowOptions {
  initialPosition?: Position;
  dragThreshold?: number;
}

export interface UseDraggableWindowReturn {
  position: Position;
  isDragging: boolean;
  isDragged: boolean;
  handleMouseDown: (e: React.MouseEvent) => void;
  handleTouchStart: (e: React.TouchEvent) => void;
  handleTouchMove: (e: React.TouchEvent) => void;
  handleTouchEnd: () => void;
  resetPosition: () => void;
}

export const useDraggableWindow = (
  options: UseDraggableWindowOptions = {}
): UseDraggableWindowReturn => {
  const {
    initialPosition = { x: 0, y: 80 },
    dragThreshold = 5,
  } = options;

  const [position, setPosition] = useState<Position>(initialPosition);
  const [isDragging, setIsDragging] = useState(false);
  const [hasMoved, setHasMoved] = useState(false);

  const dragStartRef = useRef<{
    x: number;
    y: number;
    posX: number;
    posY: number;
  } | null>(null);

  const handleDragStart = useCallback(
    (clientX: number, clientY: number) => {
      dragStartRef.current = { x: clientX, y: clientY, posX: position.x, posY: position.y };
      setHasMoved(false);
      setIsDragging(true);
    },
    [position],
  );

  const handleDragMove = useCallback(
    (clientX: number, clientY: number) => {
      if (!dragStartRef.current) return;
      const dx = clientX - dragStartRef.current.x;
      const dy = clientY - dragStartRef.current.y;
      if (Math.abs(dx) > dragThreshold || Math.abs(dy) > dragThreshold) {
        setHasMoved(true);
      }
      setPosition({
        x: dragStartRef.current.posX + dx,
        y: dragStartRef.current.posY + dy,
      });
    },
    [dragThreshold],
  );

  const handleDragEnd = useCallback(() => {
    dragStartRef.current = null;
    setIsDragging(false);
  }, []);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if ((e.target as HTMLElement).closest('.drag-handle')) {
        e.preventDefault();
        handleDragStart(e.clientX, e.clientY);
      }
    },
    [handleDragStart],
  );

  useEffect(() => {
    if (!isDragging) return;

    const onMouseMove = (e: MouseEvent) => handleDragMove(e.clientX, e.clientY);
    const onMouseUp = () => handleDragEnd();

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);

    return () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
    };
  }, [isDragging, handleDragMove, handleDragEnd]);

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if ((e.target as HTMLElement).closest('.drag-handle')) {
        const t = e.touches[0];
        handleDragStart(t.clientX, t.clientY);
      }
    },
    [handleDragStart],
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      const t = e.touches[0];
      handleDragMove(t.clientX, t.clientY);
    },
    [handleDragMove],
  );

  const resetPosition = useCallback(() => {
    setPosition(initialPosition);
    setHasMoved(false);
  }, [initialPosition]);

  return {
    position,
    isDragging,
    isDragged: hasMoved,
    handleMouseDown,
    handleTouchStart,
    handleTouchMove,
    handleTouchEnd: handleDragEnd,
    resetPosition,
  };
};
