import React, { Component } from "react";

interface ErrorBoundaryProps {
  children: React.ReactElement; // Ensures children is a React element
  onError: (error: Error) => void; // The onError prop is required
}

interface ErrorBoundaryState {
  hasError: boolean;
}

class CustomErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ErrorBoundaryState {
    // Update state to show fallback UI on error
    return { hasError: true };
  }

  componentDidCatch(error: Error): void {
    // Call the onError prop with the error
    this.props.onError(error);
  }

  render() {
    if (this.state.hasError) {
      return <div>Something went wrong.</div>; // Fallback UI
    }

    return this.props.children; // Render children if no error
  }
}

export default CustomErrorBoundary;
