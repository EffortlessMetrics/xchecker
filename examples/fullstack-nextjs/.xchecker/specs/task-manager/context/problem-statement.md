# Problem Statement: Task Manager Application

## Overview

Build a modern task management web application using Next.js 14 with the following capabilities:

- User authentication with NextAuth.js
- Create, read, update, and delete tasks
- Task organization with projects and tags
- Real-time updates using Server-Sent Events
- Responsive design with Tailwind CSS
- PostgreSQL database with Prisma ORM

## Goals

1. **User Management**
   - Email/password authentication
   - OAuth providers (Google, GitHub)
   - User profile management
   - Session handling

2. **Task Management**
   - CRUD operations for tasks
   - Task status (todo, in-progress, done)
   - Due dates and reminders
   - Priority levels (low, medium, high)
   - Task descriptions with markdown support

3. **Organization**
   - Projects to group related tasks
   - Tags for cross-project categorization
   - Filtering and search
   - Sorting options

4. **User Experience**
   - Responsive design (mobile-first)
   - Keyboard shortcuts
   - Drag-and-drop task reordering
   - Dark/light theme support

5. **Technical Quality**
   - TypeScript throughout
   - Comprehensive test coverage
   - API documentation
   - Performance optimization

## Constraints

- Use Next.js 14 App Router (not Pages Router)
- TypeScript strict mode enabled
- Follow Next.js best practices for data fetching
- Implement proper error boundaries
- Ensure WCAG 2.1 AA accessibility compliance
- Support latest 2 versions of major browsers

## Success Criteria

- All CRUD operations functional
- Authentication working with at least 2 providers
- Mobile-responsive design
- Core Web Vitals passing
- Test coverage > 80%
- API documentation complete
- CI/CD pipeline operational
