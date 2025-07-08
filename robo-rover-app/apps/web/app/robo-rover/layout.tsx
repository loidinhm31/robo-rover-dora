import type {Metadata} from 'next';

export const metadata: Metadata = {
    title: 'Robo Rover Controller',
    description: 'Mobile-first controller for Robo Rover ARM and ROVER systems',
    viewport: 'width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no',
};

export default function RoboRoverLayout({
                                            children,
                                        }: {
    children: React.ReactNode;
}) {
    return (
        <div className="min-h-screen">
            {children}
        </div>
    );
}